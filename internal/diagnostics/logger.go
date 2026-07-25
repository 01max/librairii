package diagnostics

import (
	"bufio"
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sync"
	"time"
)

const currentLogName = "events.jsonl"

type Policy struct {
	MaxFiles int
	MaxBytes int64
}

func DefaultPolicy() Policy {
	return Policy{
		MaxFiles: 5,
		MaxBytes: 256 * 1024,
	}
}

type Level string

const (
	LevelInfo  Level = "info"
	LevelWarn  Level = "warn"
	LevelError Level = "error"
)

type Event string

const (
	EventRuntimeStarted    Event = "runtime_started"
	EventRuntimeStopped    Event = "runtime_stopped"
	EventOperationChanged  Event = "operation_changed"
	EventDiagnosticsExport Event = "diagnostics_exported"
)

type Entry struct {
	Time  string `json:"time"`
	Level Level  `json:"level"`
	Event Event  `json:"event"`
	State string `json:"state,omitempty"`
}

type Logger struct {
	mu        sync.Mutex
	directory string
	policy    Policy
	now       func() time.Time
	file      *os.File
}

func NewLogger(
	directory string,
	policy Policy,
	now func() time.Time,
) (*Logger, error) {
	if !filepath.IsAbs(directory) ||
		policy.MaxFiles < 1 ||
		policy.MaxBytes < 128 ||
		now == nil {
		return nil, errors.New("diagnostic logger configuration is invalid")
	}
	info, err := os.Stat(directory)
	if err != nil || !info.IsDir() {
		return nil, errors.New("diagnostic log directory is invalid")
	}
	logger := &Logger{
		directory: directory,
		policy:    policy,
		now:       now,
	}
	file, err := logger.openCurrent()
	if err != nil {
		return nil, err
	}
	logger.file = file
	info, err = file.Stat()
	if err != nil {
		_ = file.Close()
		return nil, fmt.Errorf("inspect diagnostic log: %w", err)
	}
	if info.Size() > policy.MaxBytes {
		if err := logger.rotateLocked(); err != nil {
			_ = logger.file.Close()
			return nil, err
		}
	}
	return logger, nil
}

func (l *Logger) Record(level Level, event Event, state string) error {
	if !validLevel(level) || !validEvent(event) || !validState(state) {
		return errors.New("diagnostic entry is invalid")
	}
	entry := Entry{
		Time:  l.now().UTC().Format(time.RFC3339Nano),
		Level: level,
		Event: event,
		State: state,
	}
	payload, err := json.Marshal(entry)
	if err != nil {
		return err
	}
	payload = append(payload, '\n')
	if int64(len(payload)) > l.policy.MaxBytes {
		return errors.New("diagnostic entry exceeds the retention limit")
	}

	l.mu.Lock()
	defer l.mu.Unlock()
	if l.file == nil {
		return errors.New("diagnostic logger is closed")
	}
	info, err := l.file.Stat()
	if err != nil {
		return fmt.Errorf("inspect diagnostic log: %w", err)
	}
	if info.Size()+int64(len(payload)) > l.policy.MaxBytes {
		if err := l.rotateLocked(); err != nil {
			return err
		}
	}
	if _, err := l.file.Write(payload); err != nil {
		return fmt.Errorf("append diagnostic event: %w", err)
	}
	if err := l.file.Sync(); err != nil {
		return fmt.Errorf("sync diagnostic event: %w", err)
	}
	return nil
}

func (l *Logger) Close() error {
	l.mu.Lock()
	defer l.mu.Unlock()
	if l.file == nil {
		return nil
	}
	err := l.file.Close()
	l.file = nil
	return err
}

type logSnapshot struct {
	name  string
	bytes []byte
}

func (l *Logger) snapshots() ([]logSnapshot, error) {
	l.mu.Lock()
	defer l.mu.Unlock()
	if l.file == nil {
		return nil, errors.New("diagnostic logger is closed")
	}
	if err := l.file.Sync(); err != nil {
		return nil, fmt.Errorf("sync diagnostic log snapshot: %w", err)
	}
	files := make([]logSnapshot, 0, l.policy.MaxFiles)
	for index := 0; index < l.policy.MaxFiles; index++ {
		name := logName(index)
		payload, err := os.ReadFile(filepath.Join(l.directory, name))
		if errors.Is(err, os.ErrNotExist) {
			continue
		}
		if err != nil {
			return nil, fmt.Errorf("read diagnostic log snapshot: %w", err)
		}
		sanitized := sanitizeLog(payload)
		if len(sanitized) == 0 {
			continue
		}
		files = append(files, logSnapshot{name: name, bytes: sanitized})
	}
	return files, nil
}

func (l *Logger) rotateLocked() error {
	if l.file != nil {
		if err := l.file.Close(); err != nil {
			return fmt.Errorf("close diagnostic log for rotation: %w", err)
		}
		l.file = nil
	}
	if l.policy.MaxFiles == 1 {
		if err := os.Remove(filepath.Join(l.directory, currentLogName)); err != nil &&
			!errors.Is(err, os.ErrNotExist) {
			return fmt.Errorf("remove expired diagnostic log: %w", err)
		}
	} else {
		oldest := filepath.Join(l.directory, logName(l.policy.MaxFiles-1))
		if err := os.Remove(oldest); err != nil && !errors.Is(err, os.ErrNotExist) {
			return fmt.Errorf("remove expired diagnostic log: %w", err)
		}
		for index := l.policy.MaxFiles - 2; index >= 1; index-- {
			source := filepath.Join(l.directory, logName(index))
			target := filepath.Join(l.directory, logName(index+1))
			if err := os.Rename(source, target); err != nil &&
				!errors.Is(err, os.ErrNotExist) {
				return fmt.Errorf("rotate diagnostic log: %w", err)
			}
		}
		if err := os.Rename(
			filepath.Join(l.directory, currentLogName),
			filepath.Join(l.directory, logName(1)),
		); err != nil && !errors.Is(err, os.ErrNotExist) {
			return fmt.Errorf("rotate current diagnostic log: %w", err)
		}
	}
	file, err := l.openCurrent()
	if err != nil {
		return err
	}
	l.file = file
	return nil
}

func (l *Logger) openCurrent() (*os.File, error) {
	file, err := os.OpenFile(
		filepath.Join(l.directory, currentLogName),
		os.O_APPEND|os.O_CREATE|os.O_WRONLY,
		0o600,
	)
	if err != nil {
		return nil, fmt.Errorf("open diagnostic log: %w", err)
	}
	if err := file.Chmod(0o600); err != nil {
		_ = file.Close()
		return nil, fmt.Errorf("protect diagnostic log: %w", err)
	}
	return file, nil
}

func logName(index int) string {
	if index == 0 {
		return currentLogName
	}
	return fmt.Sprintf("events.%d.jsonl", index)
}

func sanitizeLog(payload []byte) []byte {
	var output bytes.Buffer
	scanner := bufio.NewScanner(bytes.NewReader(payload))
	scanner.Buffer(make([]byte, 1024), 16*1024)
	for scanner.Scan() {
		decoder := json.NewDecoder(bytes.NewReader(scanner.Bytes()))
		decoder.DisallowUnknownFields()
		var entry Entry
		if err := decoder.Decode(&entry); err != nil ||
			!validTimestamp(entry.Time) ||
			!validLevel(entry.Level) ||
			!validEvent(entry.Event) ||
			!validState(entry.State) {
			continue
		}
		var trailing any
		if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
			continue
		}
		encoded, err := json.Marshal(entry)
		if err != nil {
			continue
		}
		output.Write(encoded)
		output.WriteByte('\n')
	}
	return output.Bytes()
}

func validTimestamp(value string) bool {
	_, err := time.Parse(time.RFC3339Nano, value)
	return err == nil
}

func validLevel(value Level) bool {
	return value == LevelInfo || value == LevelWarn || value == LevelError
}

func validEvent(value Event) bool {
	switch value {
	case EventRuntimeStarted,
		EventRuntimeStopped,
		EventOperationChanged,
		EventDiagnosticsExport:
		return true
	default:
		return false
	}
}

func validState(value string) bool {
	if len(value) > 64 {
		return false
	}
	for _, character := range value {
		if (character >= 'a' && character <= 'z') ||
			(character >= '0' && character <= '9') ||
			character == '_' ||
			character == '.' ||
			character == ':' ||
			character == '-' {
			continue
		}
		return false
	}
	return true
}
