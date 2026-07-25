package storage

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

type SchemaState string

const (
	SchemaAbsent     SchemaState = "absent"
	SchemaCompatible SchemaState = "compatible"
	SchemaForeign    SchemaState = "foreign"
)

type SchemaIdentity struct {
	State SchemaState
}

type SchemaProbe interface {
	Inspect(context.Context, string) (SchemaIdentity, error)
}

type ReadinessIssueCode string

const (
	IssuePathEscape         ReadinessIssueCode = "path_escape"
	IssueSchemaMismatch     ReadinessIssueCode = "schema_mismatch"
	IssueStorageNotWritable ReadinessIssueCode = "storage_not_writable"
)

type ReadinessIssue struct {
	Code ReadinessIssueCode
}

type ReadinessReport struct {
	MutationsAllowed bool
	Issues           []ReadinessIssue
	Schema           SchemaIdentity
}

type ReadinessChecker struct {
	layout        Layout
	schema        SchemaProbe
	probeWritable func(string) error
}

func NewReadinessChecker(layout Layout, schema SchemaProbe) *ReadinessChecker {
	return &ReadinessChecker{
		layout:        layout,
		schema:        schema,
		probeWritable: probeDirectoryWritable,
	}
}

func (c *ReadinessChecker) Check(ctx context.Context) (ReadinessReport, error) {
	report := ReadinessReport{MutationsAllowed: true}

	for _, directory := range c.layout.Directories() {
		contained, err := PathContained(c.layout.Root, directory)
		if err != nil {
			return ReadinessReport{}, err
		}
		if !contained {
			report.MutationsAllowed = false
			report.Issues = append(report.Issues, ReadinessIssue{Code: IssuePathEscape})
			continue
		}
		if err := c.probeWritable(directory); err != nil {
			report.MutationsAllowed = false
			report.Issues = append(report.Issues, ReadinessIssue{Code: IssueStorageNotWritable})
		}
	}

	identity, err := c.schema.Inspect(ctx, filepath.Join(c.layout.Database, "librairii.sqlite3"))
	if err != nil {
		return ReadinessReport{}, fmt.Errorf("inspect database identity: %w", err)
	}
	report.Schema = identity
	if identity.State == SchemaForeign {
		report.MutationsAllowed = false
		report.Issues = append(report.Issues, ReadinessIssue{Code: IssueSchemaMismatch})
	}

	return report, nil
}

func PathContained(root string, candidate string) (bool, error) {
	cleanRoot, err := filepath.Abs(root)
	if err != nil {
		return false, err
	}
	cleanCandidate, err := filepath.Abs(candidate)
	if err != nil {
		return false, err
	}

	resolvedRoot, err := filepath.EvalSymlinks(cleanRoot)
	if err != nil {
		return false, err
	}
	resolvedCandidate, err := filepath.EvalSymlinks(cleanCandidate)
	if err != nil {
		return false, err
	}

	relative, err := filepath.Rel(resolvedRoot, resolvedCandidate)
	if err != nil {
		return false, err
	}
	return relative == "." || (relative != ".." && !strings.HasPrefix(relative, ".."+string(filepath.Separator))), nil
}

func probeDirectoryWritable(directory string) error {
	file, err := os.CreateTemp(directory, ".librairii-readiness-*")
	if err != nil {
		return err
	}
	name := file.Name()
	if closeErr := file.Close(); closeErr != nil {
		_ = os.Remove(name)
		return closeErr
	}
	return os.Remove(name)
}

type FileSchemaProbe struct{}

func (FileSchemaProbe) Inspect(_ context.Context, databasePath string) (SchemaIdentity, error) {
	info, err := os.Stat(databasePath)
	if errors.Is(err, os.ErrNotExist) {
		return SchemaIdentity{State: SchemaAbsent}, nil
	}
	if err != nil {
		return SchemaIdentity{}, err
	}
	if !info.Mode().IsRegular() {
		return SchemaIdentity{}, fmt.Errorf("database path is not a regular file")
	}

	// Until the SQLite-backed probe is installed, every pre-existing database
	// is treated as foreign rather than being opened or overwritten.
	return SchemaIdentity{State: SchemaForeign}, nil
}
