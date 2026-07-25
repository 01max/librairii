package operations

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"time"

	"github.com/01max/librairii/internal/importer"
	"github.com/01max/librairii/internal/inspection"
	"github.com/01max/librairii/internal/metadata"
	"github.com/google/uuid"
)

const (
	EventChanged          = "operation:changed"
	maxWorkerCount        = 8
	maxImportItems        = 1_000
	persistenceAttempts   = 3
	persistenceRetryDelay = 10 * time.Millisecond
	defaultRecoveryDelay  = time.Second
	outcomeImported       = "imported"
	exportProgressStep    = 256 * 1024
)

var (
	ErrManagerNotStarted = errors.New("operation manager is not started")
	ErrInvalidRequest    = errors.New("operation request is invalid")
	ErrOperationActive   = errors.New("an operation of this kind is already active")
)

type Clock interface {
	Now() time.Time
}

type EventPort interface {
	Emit(context.Context, string, any)
}

type ImportService interface {
	Import(context.Context, string) (importer.Outcome, error)
}

type ExportService interface {
	Copy(
		context.Context,
		NewItem,
		string,
		func(int64),
	) (ExportCopyResult, error)
}

type ExportRecovery interface {
	CleanupAbandoned(string) error
}

type StagingCleaner interface {
	CleanupAbandoned() error
}

type MetadataRefreshService interface {
	Refresh(context.Context, string) (metadata.RefreshResult, error)
}

type managerRepository interface {
	CreateImport(context.Context, string, []NewItem, time.Time) (Snapshot, error)
	CreateMetadataSync(context.Context, string, string, time.Time) (Snapshot, error)
	CreateExport(
		context.Context,
		string,
		ExportSource,
		string,
		[]NewItem,
		time.Time,
	) (Snapshot, error)
	MarkRunning(context.Context, string, time.Time) error
	MarkItemRunning(context.Context, int64) error
	UpdateItemProgress(context.Context, string, int64, int64) error
	CompleteItem(context.Context, string, ItemSnapshot) error
	RequestCancel(context.Context, string) error
	Finish(context.Context, string, Status, string, string, time.Time) error
	Interrupt(context.Context, string, string, string, time.Time) (Snapshot, error)
	Snapshot(context.Context, string) (Snapshot, error)
	ActiveSnapshots(context.Context) ([]Snapshot, error)
	LatestTerminalExport(context.Context) (Snapshot, bool, error)
}

type Dependencies struct {
	Repository     managerRepository
	Imports        ImportService
	Exports        ExportService
	ExportRecovery ExportRecovery
	Metadata       MetadataRefreshService
	Events         EventPort
	Clock          Clock
	StagingCleaner StagingCleaner
	Workers        int
	RecoveryDelay  time.Duration
}

type Manager struct {
	mu             sync.Mutex
	deps           Dependencies
	started        bool
	ctx            context.Context
	cancel         context.CancelFunc
	jobs           chan operationJob
	runs           map[string]*operationRun
	wg             sync.WaitGroup
	recoveryDelay  time.Duration
	metadataActive bool
}

type operationRun struct {
	id        string
	kind      Kind
	ctx       context.Context
	cancel    context.CancelFunc
	startOnce sync.Once
	stopOnce  sync.Once
	stopped   chan struct{}
	startErr  error

	mu             sync.Mutex
	total          int
	remaining      int
	successes      int
	failures       int
	cancelled      int
	skipped        int
	conflicted     int
	failureCode    string
	failureMessage string
}

type operationJob struct {
	run         *operationRun
	item        ItemSnapshot
	source      string
	locale      string
	total       int64
	export      ExportWorkItem
	destination string
}

func NewManager(deps Dependencies) (*Manager, error) {
	if deps.Repository == nil ||
		deps.Imports == nil ||
		deps.Events == nil ||
		deps.Clock == nil ||
		deps.StagingCleaner == nil {
		return nil, fmt.Errorf("operation manager dependency is nil")
	}
	if deps.Workers < 1 || deps.Workers > maxWorkerCount {
		return nil, fmt.Errorf("%w: worker count", ErrInvalidRequest)
	}
	recoveryDelay := deps.RecoveryDelay
	if recoveryDelay <= 0 {
		recoveryDelay = defaultRecoveryDelay
	}
	return &Manager{
		deps:          deps,
		runs:          make(map[string]*operationRun),
		recoveryDelay: recoveryDelay,
	}, nil
}

func (m *Manager) Start(ctx context.Context) error {
	m.mu.Lock()
	if m.started {
		m.mu.Unlock()
		return nil
	}
	m.mu.Unlock()

	active, err := m.deps.Repository.ActiveSnapshots(ctx)
	if err != nil {
		return fmt.Errorf("list interrupted operations: %w", err)
	}
	if err := m.deps.StagingCleaner.CleanupAbandoned(); err != nil {
		return fmt.Errorf("clean abandoned staging: %w", err)
	}
	for _, snapshot := range active {
		code, message := interruptionOutcome(snapshot.Kind)
		if snapshot.Kind == KindExport {
			if m.deps.ExportRecovery == nil {
				return fmt.Errorf("export recovery dependency is nil")
			}
			if err := m.deps.ExportRecovery.CleanupAbandoned(
				snapshot.Destination,
			); err != nil {
				code = "interrupted_cleanup_failed"
				message = "Export stopped when the application closed. Completed files were preserved, but an abandoned temporary file could not be removed. Check the destination folder before retrying."
			}
		}
		reconciled, err := m.deps.Repository.Interrupt(
			ctx,
			snapshot.ID,
			code,
			message,
			m.deps.Clock.Now(),
		)
		if err != nil {
			return fmt.Errorf("reconcile interrupted operation %s: %w", snapshot.ID, err)
		}
		m.deps.Events.Emit(ctx, EventChanged, reconciled)
	}

	m.mu.Lock()
	defer m.mu.Unlock()
	if m.started {
		return nil
	}
	// Worker events must retain the Wails lifecycle context stored by the
	// composition root. A background context loses Wails' runtime values and
	// makes EventsEmit terminate the packaged application.
	m.ctx, m.cancel = context.WithCancel(ctx)
	m.jobs = make(chan operationJob, m.deps.Workers*2)
	m.started = true
	for range m.deps.Workers {
		m.wg.Add(1)
		go m.worker()
	}
	return nil
}

func (m *Manager) Close() error {
	m.mu.Lock()
	if !m.started {
		m.mu.Unlock()
		return nil
	}
	m.started = false
	for _, run := range m.runs {
		run.cancel()
	}
	m.cancel()
	m.mu.Unlock()
	m.wg.Wait()
	return nil
}

func (m *Manager) StartImport(
	ctx context.Context,
	sourcePaths []string,
) (Snapshot, error) {
	if len(sourcePaths) == 0 || len(sourcePaths) > maxImportItems {
		return Snapshot{}, ErrInvalidRequest
	}

	m.mu.Lock()
	if !m.started {
		m.mu.Unlock()
		return Snapshot{}, ErrManagerNotStarted
	}
	managerContext := m.ctx
	m.mu.Unlock()

	items := make([]NewItem, 0, len(sourcePaths))
	for _, sourcePath := range sourcePaths {
		sourceName := filepath.Base(sourcePath)
		if sourcePath == "" || sourceName == "." || sourceName == string(filepath.Separator) {
			return Snapshot{}, ErrInvalidRequest
		}
		var totalBytes int64
		if info, err := os.Stat(sourcePath); err == nil && info.Mode().IsRegular() {
			totalBytes = info.Size()
		}
		items = append(items, NewItem{SourceName: sourceName, TotalBytes: totalBytes})
	}

	operationID := uuid.NewString()
	snapshot, err := m.deps.Repository.CreateImport(
		ctx,
		operationID,
		items,
		m.deps.Clock.Now(),
	)
	if err != nil {
		return Snapshot{}, err
	}
	runContext, cancel := context.WithCancel(managerContext)
	run := &operationRun{
		id:        operationID,
		kind:      KindImport,
		ctx:       runContext,
		cancel:    cancel,
		stopped:   make(chan struct{}),
		total:     len(sourcePaths),
		remaining: len(sourcePaths),
	}

	m.mu.Lock()
	if !m.started {
		m.mu.Unlock()
		cancel()
		return Snapshot{}, ErrManagerNotStarted
	}
	m.runs[operationID] = run
	m.mu.Unlock()

	m.deps.Events.Emit(ctx, EventChanged, snapshot)
	go m.enqueue(run, snapshot.Items, sourcePaths)
	return snapshot, nil
}

func (m *Manager) StartMetadataRefresh(
	ctx context.Context,
	locale string,
) (Snapshot, error) {
	if locale == "" {
		return Snapshot{}, ErrInvalidRequest
	}
	m.mu.Lock()
	if !m.started {
		m.mu.Unlock()
		return Snapshot{}, ErrManagerNotStarted
	}
	if m.deps.Metadata == nil {
		m.mu.Unlock()
		return Snapshot{}, ErrInvalidRequest
	}
	if m.metadataActive {
		m.mu.Unlock()
		return Snapshot{}, ErrOperationActive
	}
	m.metadataActive = true
	managerContext := m.ctx
	m.mu.Unlock()

	operationID := uuid.NewString()
	snapshot, err := m.deps.Repository.CreateMetadataSync(
		ctx,
		operationID,
		locale,
		m.deps.Clock.Now(),
	)
	if err != nil {
		m.mu.Lock()
		m.metadataActive = false
		m.mu.Unlock()
		return Snapshot{}, err
	}
	runContext, cancel := context.WithCancel(managerContext)
	run := &operationRun{
		id:        operationID,
		kind:      KindMetadataSync,
		ctx:       runContext,
		cancel:    cancel,
		stopped:   make(chan struct{}),
		total:     1,
		remaining: 1,
	}

	m.mu.Lock()
	if !m.started {
		m.metadataActive = false
		m.mu.Unlock()
		cancel()
		return Snapshot{}, ErrManagerNotStarted
	}
	m.runs[operationID] = run
	m.mu.Unlock()

	m.deps.Events.Emit(ctx, EventChanged, snapshot)
	go m.enqueueMetadata(run, snapshot.Items[0], locale)
	return snapshot, nil
}

func (m *Manager) StartExport(
	ctx context.Context,
	source ExportSource,
	destination string,
	workItems []ExportWorkItem,
) (Snapshot, error) {
	if len(workItems) == 0 {
		return Snapshot{}, ErrInvalidRequest
	}
	items := make([]NewItem, 0, len(workItems))
	readyItems := 0
	for _, work := range workItems {
		switch work.PlannedStatus {
		case ItemPending:
			readyItems++
		case ItemSkipped, ItemConflicted:
			if work.OutcomeCode == "" || work.OutcomeMessage == "" {
				return Snapshot{}, ErrInvalidRequest
			}
		default:
			return Snapshot{}, ErrInvalidRequest
		}
		item := work.Item
		if work.PlannedStatus != ItemPending {
			item.TotalBytes = 0
		}
		items = append(items, item)
	}
	if readyItems == 0 {
		return Snapshot{}, ErrInvalidRequest
	}

	m.mu.Lock()
	if !m.started {
		m.mu.Unlock()
		return Snapshot{}, ErrManagerNotStarted
	}
	if m.deps.Exports == nil {
		m.mu.Unlock()
		return Snapshot{}, ErrInvalidRequest
	}
	managerContext := m.ctx
	m.mu.Unlock()

	operationID := uuid.NewString()
	snapshot, err := m.deps.Repository.CreateExport(
		ctx,
		operationID,
		source,
		destination,
		items,
		m.deps.Clock.Now(),
	)
	if err != nil {
		return Snapshot{}, err
	}
	runContext, cancel := context.WithCancel(managerContext)
	run := &operationRun{
		id:        operationID,
		kind:      KindExport,
		ctx:       runContext,
		cancel:    cancel,
		stopped:   make(chan struct{}),
		total:     len(workItems),
		remaining: len(workItems),
	}

	m.mu.Lock()
	if !m.started {
		m.mu.Unlock()
		cancel()
		return Snapshot{}, ErrManagerNotStarted
	}
	m.runs[operationID] = run
	m.mu.Unlock()

	m.deps.Events.Emit(ctx, EventChanged, snapshot)
	go m.enqueueExport(run, snapshot.Items, workItems, destination)
	return snapshot, nil
}

func (m *Manager) Cancel(ctx context.Context, operationID string) (Snapshot, error) {
	if err := m.deps.Repository.RequestCancel(ctx, operationID); err != nil {
		return Snapshot{}, err
	}
	m.mu.Lock()
	run := m.runs[operationID]
	m.mu.Unlock()
	if run != nil {
		run.cancel()
	}
	snapshot, err := m.deps.Repository.Snapshot(ctx, operationID)
	if err != nil {
		return Snapshot{}, err
	}
	m.deps.Events.Emit(ctx, EventChanged, snapshot)
	return snapshot, nil
}

func (m *Manager) Snapshot(ctx context.Context, operationID string) (Snapshot, error) {
	return m.deps.Repository.Snapshot(ctx, operationID)
}

func (m *Manager) Active(ctx context.Context) ([]Snapshot, error) {
	snapshots, err := m.deps.Repository.ActiveSnapshots(ctx)
	if err != nil {
		return nil, err
	}
	report, found, err := m.deps.Repository.LatestTerminalExport(ctx)
	if err != nil {
		return nil, err
	}
	if found {
		snapshots = append(snapshots, report)
	}
	return snapshots, nil
}

func (m *Manager) enqueue(
	run *operationRun,
	items []ItemSnapshot,
	sourcePaths []string,
) {
	for index, item := range items {
		job := operationJob{
			run:    run,
			item:   item,
			source: sourcePaths[index],
			total:  item.TotalBytes,
		}
		select {
		case m.jobs <- job:
		case <-m.ctx.Done():
			return
		}
	}
}

func (m *Manager) enqueueExport(
	run *operationRun,
	items []ItemSnapshot,
	workItems []ExportWorkItem,
	destination string,
) {
	for index, item := range items {
		job := operationJob{
			run:         run,
			item:        item,
			total:       item.TotalBytes,
			export:      workItems[index],
			destination: destination,
		}
		select {
		case m.jobs <- job:
		case <-m.ctx.Done():
			return
		}
	}
}

func (m *Manager) enqueueMetadata(
	run *operationRun,
	item ItemSnapshot,
	locale string,
) {
	select {
	case m.jobs <- operationJob{
		run:    run,
		item:   item,
		locale: locale,
	}:
	case <-m.ctx.Done():
	}
}

func (m *Manager) worker() {
	defer m.wg.Done()
	for {
		select {
		case job := <-m.jobs:
			switch job.run.kind {
			case KindExport:
				m.executeExport(job)
			case KindMetadataSync:
				m.executeMetadata(job.run, job.item, job.locale)
			default:
				m.execute(job)
			}
		case <-m.ctx.Done():
			return
		}
	}
}

func (m *Manager) execute(job operationJob) {
	job.run.startOnce.Do(func() {
		job.run.startErr = m.deps.Repository.MarkRunning(
			m.ctx,
			job.run.id,
			m.deps.Clock.Now(),
		)
		if job.run.startErr == nil {
			m.emitSnapshot(job.run.id)
		}
	})
	if job.run.startErr != nil {
		m.complete(job.run, ItemSnapshot{
			ID:             job.item.ID,
			SourceName:     job.item.SourceName,
			Status:         ItemFailed,
			OutcomeCode:    "operation_start_failed",
			OutcomeMessage: "The operation could not start.",
			TotalBytes:     job.total,
		})
		return
	}

	if err := job.run.ctx.Err(); err != nil {
		m.complete(job.run, cancelledItem(job))
		return
	}
	if err := m.deps.Repository.MarkItemRunning(m.ctx, job.item.ID); err != nil {
		m.complete(job.run, ItemSnapshot{
			ID:             job.item.ID,
			SourceName:     job.item.SourceName,
			Status:         ItemFailed,
			OutcomeCode:    "item_start_failed",
			OutcomeMessage: "The item could not start.",
			TotalBytes:     job.total,
		})
		return
	}
	m.emitSnapshot(job.run.id)

	outcome, err := m.deps.Imports.Import(job.run.ctx, job.source)
	item := classifyImport(job, outcome, err)
	m.complete(job.run, item)
}

func (m *Manager) executeExport(job operationJob) {
	job.run.startOnce.Do(func() {
		job.run.startErr = m.deps.Repository.MarkRunning(
			m.ctx,
			job.run.id,
			m.deps.Clock.Now(),
		)
		if job.run.startErr == nil {
			m.emitSnapshot(job.run.id)
		}
	})
	if job.run.startErr != nil {
		m.complete(job.run, exportFailedItem(
			job,
			"operation_start_failed",
			"The export operation could not start.",
		))
		return
	}
	if err := job.run.ctx.Err(); err != nil {
		m.complete(job.run, cancelledExportItem(job))
		return
	}
	if job.export.PlannedStatus != ItemPending {
		item := exportItemSnapshot(job)
		item.Status = job.export.PlannedStatus
		item.OutcomeCode = job.export.OutcomeCode
		item.OutcomeMessage = job.export.OutcomeMessage
		m.complete(job.run, item)
		return
	}
	if err := m.deps.Repository.MarkItemRunning(m.ctx, job.item.ID); err != nil {
		m.complete(job.run, exportFailedItem(
			job,
			"item_start_failed",
			"The story export could not start.",
		))
		return
	}
	m.emitSnapshot(job.run.id)

	var completedBytes int64
	var persistedBytes int64
	var progressErr error
	var progressMu sync.Mutex
	result, err := m.deps.Exports.Copy(
		job.run.ctx,
		job.export.Item,
		job.destination,
		func(delta int64) {
			progressMu.Lock()
			defer progressMu.Unlock()
			if progressErr != nil || delta <= 0 {
				return
			}
			completedBytes += delta
			if completedBytes-persistedBytes < exportProgressStep &&
				completedBytes < job.total {
				return
			}
			progressErr = m.deps.Repository.UpdateItemProgress(
				m.ctx,
				job.run.id,
				job.item.ID,
				completedBytes,
			)
			if progressErr != nil {
				job.run.cancel()
				return
			}
			persistedBytes = completedBytes
			m.emitSnapshot(job.run.id)
		},
	)
	progressMu.Lock()
	savedProgressErr := progressErr
	progressMu.Unlock()
	if savedProgressErr != nil {
		m.interruptForPersistence(job.run)
		return
	}
	m.complete(job.run, classifyExport(job, result, err))
}

func (m *Manager) executeMetadata(
	run *operationRun,
	item ItemSnapshot,
	locale string,
) {
	run.startOnce.Do(func() {
		run.startErr = m.deps.Repository.MarkRunning(
			m.ctx,
			run.id,
			m.deps.Clock.Now(),
		)
		if run.startErr == nil {
			m.emitSnapshot(run.id)
		}
	})
	if run.startErr != nil {
		m.complete(run, ItemSnapshot{
			ID:             item.ID,
			SourceName:     item.SourceName,
			Status:         ItemFailed,
			OutcomeCode:    "operation_start_failed",
			OutcomeMessage: "The metadata refresh could not start.",
			TotalBytes:     item.TotalBytes,
		})
		return
	}
	if err := run.ctx.Err(); err != nil {
		m.complete(run, cancelledMetadataItem(item))
		return
	}
	if err := m.deps.Repository.MarkItemRunning(m.ctx, item.ID); err != nil {
		m.complete(run, ItemSnapshot{
			ID:             item.ID,
			SourceName:     item.SourceName,
			Status:         ItemFailed,
			OutcomeCode:    "item_start_failed",
			OutcomeMessage: "The metadata refresh could not start.",
			TotalBytes:     item.TotalBytes,
		})
		return
	}
	m.emitSnapshot(run.id)

	result, err := m.deps.Metadata.Refresh(run.ctx, locale)
	m.complete(run, classifyMetadataRefresh(item, result, err))
}

func (m *Manager) complete(run *operationRun, item ItemSnapshot) {
	select {
	case <-run.stopped:
		return
	default:
	}
	if err := retryPersistence(m.ctx, func() error {
		return m.deps.Repository.CompleteItem(m.ctx, run.id, item)
	}); err != nil {
		m.interruptForPersistence(run)
		return
	}
	remaining := run.record(item)
	if remaining == 0 {
		status, code, message := run.finalStatus()
		if err := retryPersistence(m.ctx, func() error {
			return m.deps.Repository.Finish(
				m.ctx,
				run.id,
				status,
				code,
				message,
				m.deps.Clock.Now(),
			)
		}); err != nil {
			m.interruptForPersistence(run)
			return
		}
		m.finishRun(run)
	}
	m.emitSnapshot(run.id)
}

func (m *Manager) interruptForPersistence(run *operationRun) {
	run.stopOnce.Do(func() {
		close(run.stopped)
		run.cancel()
		const (
			errorCode    = "persistence_failed"
			errorMessage = "Operation progress could not be saved."
		)
		var snapshot Snapshot
		err := retryPersistence(m.ctx, func() error {
			var interruptErr error
			snapshot, interruptErr = m.deps.Repository.Interrupt(
				m.ctx,
				run.id,
				errorCode,
				errorMessage,
				m.deps.Clock.Now(),
			)
			return interruptErr
		})
		if err != nil {
			snapshot = m.persistenceFailureSnapshot(run, errorCode, errorMessage)
			m.deps.Events.Emit(m.ctx, EventChanged, snapshot)
			m.wg.Add(1)
			go m.recoverInterruption(run, errorCode, errorMessage)
			return
		}
		m.removeRun(run.id)
		m.deps.Events.Emit(m.ctx, EventChanged, snapshot)
	})
}

func (m *Manager) recoverInterruption(
	run *operationRun,
	errorCode string,
	errorMessage string,
) {
	defer m.wg.Done()
	timer := time.NewTimer(m.recoveryDelay)
	defer timer.Stop()
	for {
		select {
		case <-m.ctx.Done():
			return
		case <-timer.C:
		}

		snapshot, err := m.deps.Repository.Interrupt(
			m.ctx,
			run.id,
			errorCode,
			errorMessage,
			m.deps.Clock.Now(),
		)
		if err == nil {
			m.removeRun(run.id)
			m.deps.Events.Emit(m.ctx, EventChanged, snapshot)
			return
		}
		snapshot, snapshotErr := m.deps.Repository.Snapshot(m.ctx, run.id)
		if snapshotErr == nil && snapshot.Terminal() {
			m.removeRun(run.id)
			m.deps.Events.Emit(m.ctx, EventChanged, snapshot)
			return
		}
		timer.Reset(m.recoveryDelay)
	}
}

func (m *Manager) persistenceFailureSnapshot(
	run *operationRun,
	errorCode string,
	errorMessage string,
) Snapshot {
	snapshot, err := m.deps.Repository.Snapshot(m.ctx, run.id)
	if err != nil {
		snapshot = Snapshot{
			ID:         run.id,
			Kind:       run.kind,
			TotalItems: run.total,
		}
	}
	snapshot.Status = StatusInterrupted
	snapshot.CompletedItems = snapshot.TotalItems
	snapshot.ErrorCode = errorCode
	snapshot.ErrorMessage = errorMessage
	snapshot.FinishedAt = formatTime(m.deps.Clock.Now())
	for index := range snapshot.Items {
		switch snapshot.Items[index].Status {
		case ItemPending, ItemRunning:
			snapshot.Items[index].Status = ItemCancelled
			snapshot.Items[index].OutcomeCode = errorCode
			snapshot.Items[index].OutcomeMessage = errorMessage
		}
	}
	return snapshot
}

func (m *Manager) finishRun(run *operationRun) {
	run.cancel()
	m.removeRun(run.id)
}

func (m *Manager) removeRun(operationID string) {
	m.mu.Lock()
	if run := m.runs[operationID]; run != nil && run.kind == KindMetadataSync {
		m.metadataActive = false
	}
	delete(m.runs, operationID)
	m.mu.Unlock()
}

func retryPersistence(ctx context.Context, action func() error) error {
	var lastErr error
	for attempt := 0; attempt < persistenceAttempts; attempt++ {
		if lastErr = action(); lastErr == nil {
			return nil
		}
		if attempt == persistenceAttempts-1 {
			break
		}
		timer := time.NewTimer(persistenceRetryDelay)
		select {
		case <-ctx.Done():
			timer.Stop()
			return errors.Join(lastErr, ctx.Err())
		case <-timer.C:
		}
	}
	return lastErr
}

func (r *operationRun) record(item ItemSnapshot) int {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.remaining--
	if r.kind == KindExport {
		switch item.Status {
		case ItemSucceeded:
			r.successes++
		case ItemSkipped:
			r.skipped++
		case ItemConflicted:
			r.conflicted++
		case ItemCancelled:
			r.cancelled++
		default:
			r.failures++
			if r.failureCode == "" {
				r.failureCode = item.OutcomeCode
				r.failureMessage = item.OutcomeMessage
			}
		}
		return r.remaining
	}
	switch item.Status {
	case ItemSucceeded, ItemSkipped:
		r.successes++
	case ItemCancelled:
		r.cancelled++
	default:
		r.failures++
		if r.failureCode == "" {
			r.failureCode = item.OutcomeCode
			r.failureMessage = item.OutcomeMessage
		}
	}
	return r.remaining
}

func (r *operationRun) finalStatus() (Status, string, string) {
	r.mu.Lock()
	defer r.mu.Unlock()
	switch {
	case r.kind == KindExport &&
		r.failures == 0 &&
		r.cancelled == 0 &&
		r.skipped == 0 &&
		r.conflicted == 0:
		return StatusSucceeded, "", ""
	case r.kind == KindExport && r.successes > 0:
		return StatusPartiallySucceeded,
			"partial_export",
			"Some stories were not exported."
	case r.kind == KindExport && r.cancelled > 0 && r.failures == 0:
		return StatusCancelled, "cancelled", "The export was cancelled."
	case r.kind == KindExport:
		code := r.failureCode
		if code == "" {
			code = "export_failed"
		}
		message := r.failureMessage
		if message == "" {
			message = "No stories were exported."
		}
		return StatusFailed, code, message
	case r.failures == 0 && r.cancelled == 0:
		return StatusSucceeded, "", ""
	case r.kind == KindMetadataSync && r.failures == 0:
		return StatusCancelled, "catalog_cancelled", "Official metadata refresh was cancelled."
	case r.kind == KindMetadataSync:
		code := r.failureCode
		if code == "" {
			code = "catalog_refresh_failed"
		}
		message := r.failureMessage
		if message == "" {
			message = "Official metadata could not be refreshed."
		}
		return StatusFailed, code, message
	case r.successes > 0:
		return StatusPartiallySucceeded,
			"partial_failure",
			"Some files could not be imported."
	case r.failures == 0:
		return StatusCancelled, "cancelled", "The import was cancelled."
	default:
		return StatusFailed, "import_failed", "No selected files were imported."
	}
}

func classifyMetadataRefresh(
	item ItemSnapshot,
	result metadata.RefreshResult,
	err error,
) ItemSnapshot {
	item.CompletedBytes = item.TotalBytes
	if err == nil {
		item.Status = ItemSucceeded
		item.OutcomeCode = "metadata_refreshed"
		item.OutcomeMessage = fmt.Sprintf(
			"Official metadata refreshed; %d local stories matched.",
			result.Sync.MatchedStoryCount,
		)
		return item
	}
	if errors.Is(err, context.Canceled) ||
		metadata.RefreshErrorHasCode(err, metadata.RefreshCancelled) {
		return cancelledMetadataItem(item)
	}
	item.Status = ItemFailed
	item.OutcomeCode = "catalog_refresh_failed"
	item.OutcomeMessage = "Official metadata could not be refreshed."
	var refreshError *metadata.RefreshError
	if errors.As(err, &refreshError) {
		item.OutcomeCode = string(refreshError.Code)
		switch refreshError.Code {
		case metadata.RefreshFetchFailed:
			item.OutcomeMessage = "Official metadata could not be downloaded."
		case metadata.RefreshInvalidCatalog:
			item.OutcomeMessage = "The downloaded official catalog was invalid."
		case metadata.RefreshStagingFailed:
			item.OutcomeMessage = "Official metadata could not be staged safely."
		case metadata.RefreshPersistenceFailed:
			item.OutcomeMessage = "Official metadata could not be saved."
		}
	}
	return item
}

func cancelledMetadataItem(item ItemSnapshot) ItemSnapshot {
	item.Status = ItemCancelled
	item.OutcomeCode = string(metadata.RefreshCancelled)
	item.OutcomeMessage = "Official metadata refresh was cancelled."
	return item
}

func classifyExport(
	job operationJob,
	result ExportCopyResult,
	err error,
) ItemSnapshot {
	item := exportItemSnapshot(job)
	if err == nil {
		item.Status = ItemSucceeded
		item.OutcomeCode = "exported"
		item.OutcomeMessage = "Story archive exported."
		item.CompletedBytes = result.ByteSize
		return item
	}
	if errors.Is(err, context.Canceled) || result.OutcomeCode == "cancelled" {
		return cancelledExportItem(job)
	}
	switch result.OutcomeCode {
	case "filename_conflict":
		item.Status = ItemConflicted
		item.OutcomeCode = result.OutcomeCode
		item.OutcomeMessage = "A file with this name already exists in the destination."
	case "checksum_mismatch":
		item.Status = ItemFailed
		item.OutcomeCode = result.OutcomeCode
		item.OutcomeMessage = "The copied archive failed checksum verification."
	case "source_invalid":
		item.Status = ItemFailed
		item.OutcomeCode = result.OutcomeCode
		item.OutcomeMessage = "The managed archive is no longer available for export."
	default:
		item.Status = ItemFailed
		item.OutcomeCode = "export_failed"
		item.OutcomeMessage = "The story archive could not be exported."
	}
	return item
}

func exportItemSnapshot(job operationJob) ItemSnapshot {
	return ItemSnapshot{
		ID:         job.item.ID,
		StoryID:    job.export.Item.StoryID,
		StoryUUID:  job.export.Item.StoryUUID,
		StoryTitle: job.export.Item.StoryTitle,
		SourceName: job.export.Item.SourceName,
		OutputName: job.export.Item.OutputName,
		Status:     ItemPending,
		TotalBytes: job.total,
	}
}

func exportFailedItem(
	job operationJob,
	code string,
	message string,
) ItemSnapshot {
	item := exportItemSnapshot(job)
	item.Status = ItemFailed
	item.OutcomeCode = code
	item.OutcomeMessage = message
	return item
}

func cancelledExportItem(job operationJob) ItemSnapshot {
	item := exportItemSnapshot(job)
	item.Status = ItemCancelled
	item.OutcomeCode = "cancelled"
	item.OutcomeMessage = "Story export cancelled."
	return item
}

func classifyImport(job operationJob, outcome importer.Outcome, err error) ItemSnapshot {
	item := ItemSnapshot{
		ID:         job.item.ID,
		SourceName: job.item.SourceName,
		TotalBytes: job.total,
	}
	if err == nil {
		item.StoryID = outcome.StoryID
		item.CompletedBytes = job.total
		switch outcome.Code {
		case importer.OutcomeImported:
			item.Status = ItemSucceeded
			item.OutcomeCode = outcomeImported
			item.OutcomeMessage = "Story imported."
		case importer.OutcomeDuplicateChecksum:
			item.Status = ItemSkipped
			item.OutcomeCode = string(importer.OutcomeDuplicateChecksum)
			item.OutcomeMessage = "This archive is already in the collection."
		case importer.OutcomeUUIDConflict:
			item.Status = ItemConflicted
			item.OutcomeCode = string(importer.OutcomeUUIDConflict)
			item.OutcomeMessage = "Different archive bytes already use this story UUID."
		default:
			item.Status = ItemFailed
			item.OutcomeCode = "unknown_import_outcome"
			item.OutcomeMessage = "The import returned an unknown outcome."
		}
		return item
	}
	if errors.Is(err, context.Canceled) {
		return cancelledItem(job)
	}

	item.Status = ItemFailed
	item.OutcomeCode = "import_failed"
	item.OutcomeMessage = "The file could not be imported."
	var validationError *inspection.ValidationError
	if errors.As(err, &validationError) {
		item.OutcomeCode = string(validationError.Code)
		item.OutcomeMessage = validationMessage(validationError.Code)
		return item
	}
	var importError *importer.Error
	if errors.As(err, &importError) {
		item.OutcomeCode = string(importError.Code)
	}
	return item
}

func cancelledItem(job operationJob) ItemSnapshot {
	return ItemSnapshot{
		ID:             job.item.ID,
		SourceName:     job.item.SourceName,
		Status:         ItemCancelled,
		OutcomeCode:    "cancelled",
		OutcomeMessage: "Import cancelled.",
		TotalBytes:     job.total,
	}
}

func validationMessage(code inspection.ErrorCode) string {
	switch code {
	case inspection.CodeUnsafePath,
		inspection.CodeLinkEntry,
		inspection.CodeNestedArchive,
		inspection.CodeEntryLimit,
		inspection.CodeExpandedSizeLimit,
		inspection.CodeCompressionRatioLimit,
		inspection.CodeMetadataLimit,
		inspection.CodeArtworkLimit:
		return "The archive exceeds safe import limits."
	case inspection.CodeInvalidUUID:
		return "The archive does not contain a valid story UUID."
	default:
		return "The file is not a supported story archive."
	}
}

func (m *Manager) emitSnapshot(operationID string) {
	snapshot, err := m.deps.Repository.Snapshot(m.ctx, operationID)
	if err == nil {
		m.deps.Events.Emit(m.ctx, EventChanged, snapshot)
	}
}
