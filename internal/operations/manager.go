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
	"github.com/google/uuid"
)

const (
	EventChanged          = "operation:changed"
	maxWorkerCount        = 8
	maxImportItems        = 1_000
	persistenceAttempts   = 3
	persistenceRetryDelay = 10 * time.Millisecond
	outcomeImported       = "imported"
)

var (
	ErrManagerNotStarted = errors.New("operation manager is not started")
	ErrInvalidRequest    = errors.New("operation request is invalid")
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

type StagingCleaner interface {
	CleanupAbandoned() error
}

type managerRepository interface {
	CreateImport(context.Context, string, []NewItem, time.Time) (Snapshot, error)
	MarkRunning(context.Context, string, time.Time) error
	MarkItemRunning(context.Context, int64) error
	CompleteItem(context.Context, string, ItemSnapshot) error
	RequestCancel(context.Context, string) error
	Finish(context.Context, string, Status, string, string, time.Time) error
	Interrupt(context.Context, string, string, string, time.Time) (Snapshot, error)
	Snapshot(context.Context, string) (Snapshot, error)
	ActiveSnapshots(context.Context) ([]Snapshot, error)
	InterruptActive(context.Context, time.Time) ([]Snapshot, error)
}

type Dependencies struct {
	Repository     managerRepository
	Imports        ImportService
	Events         EventPort
	Clock          Clock
	StagingCleaner StagingCleaner
	Workers        int
}

type Manager struct {
	mu      sync.Mutex
	deps    Dependencies
	started bool
	ctx     context.Context
	cancel  context.CancelFunc
	jobs    chan importJob
	runs    map[string]*operationRun
	wg      sync.WaitGroup
}

type operationRun struct {
	id        string
	ctx       context.Context
	cancel    context.CancelFunc
	startOnce sync.Once
	stopOnce  sync.Once
	startErr  error

	mu        sync.Mutex
	total     int
	remaining int
	successes int
	failures  int
	cancelled int
}

type importJob struct {
	run    *operationRun
	item   ItemSnapshot
	source string
	total  int64
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
	return &Manager{
		deps: deps,
		runs: make(map[string]*operationRun),
	}, nil
}

func (m *Manager) Start(ctx context.Context) error {
	m.mu.Lock()
	if m.started {
		m.mu.Unlock()
		return nil
	}
	m.mu.Unlock()

	reconciled, err := m.deps.Repository.InterruptActive(ctx, m.deps.Clock.Now())
	if err != nil {
		return fmt.Errorf("reconcile interrupted operations: %w", err)
	}
	if err := m.deps.StagingCleaner.CleanupAbandoned(); err != nil {
		return fmt.Errorf("clean abandoned staging: %w", err)
	}
	for _, snapshot := range reconciled {
		m.deps.Events.Emit(ctx, EventChanged, snapshot)
	}

	m.mu.Lock()
	defer m.mu.Unlock()
	if m.started {
		return nil
	}
	m.ctx, m.cancel = context.WithCancel(context.Background())
	m.jobs = make(chan importJob, m.deps.Workers*2)
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
		ctx:       runContext,
		cancel:    cancel,
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
	return m.deps.Repository.ActiveSnapshots(ctx)
}

func (m *Manager) enqueue(
	run *operationRun,
	items []ItemSnapshot,
	sourcePaths []string,
) {
	for index, item := range items {
		job := importJob{
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

func (m *Manager) worker() {
	defer m.wg.Done()
	for {
		select {
		case job := <-m.jobs:
			m.execute(job)
		case <-m.ctx.Done():
			return
		}
	}
}

func (m *Manager) execute(job importJob) {
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
		m.complete(job, ItemSnapshot{
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
		m.complete(job, cancelledItem(job))
		return
	}
	if err := m.deps.Repository.MarkItemRunning(m.ctx, job.item.ID); err != nil {
		m.complete(job, ItemSnapshot{
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
	m.complete(job, item)
}

func (m *Manager) complete(job importJob, item ItemSnapshot) {
	if err := retryPersistence(m.ctx, func() error {
		return m.deps.Repository.CompleteItem(m.ctx, job.run.id, item)
	}); err != nil {
		m.interruptForPersistence(job.run)
		return
	}
	remaining := job.run.record(item.Status)
	if remaining == 0 {
		status, code, message := job.run.finalStatus()
		if err := retryPersistence(m.ctx, func() error {
			return m.deps.Repository.Finish(
				m.ctx,
				job.run.id,
				status,
				code,
				message,
				m.deps.Clock.Now(),
			)
		}); err != nil {
			m.interruptForPersistence(job.run)
			return
		}
		m.finishRun(job.run)
	}
	m.emitSnapshot(job.run.id)
}

func (m *Manager) interruptForPersistence(run *operationRun) {
	run.stopOnce.Do(func() {
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
		}
		m.removeRun(run.id)
		m.deps.Events.Emit(m.ctx, EventChanged, snapshot)
	})
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
			Kind:       KindImport,
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

func (r *operationRun) record(status ItemStatus) int {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.remaining--
	switch status {
	case ItemSucceeded, ItemSkipped:
		r.successes++
	case ItemCancelled:
		r.cancelled++
	default:
		r.failures++
	}
	return r.remaining
}

func (r *operationRun) finalStatus() (Status, string, string) {
	r.mu.Lock()
	defer r.mu.Unlock()
	switch {
	case r.failures == 0 && r.cancelled == 0:
		return StatusSucceeded, "", ""
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

func classifyImport(job importJob, outcome importer.Outcome, err error) ItemSnapshot {
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

func cancelledItem(job importJob) ItemSnapshot {
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
