package storage

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"testing"
)

type fixedSchemaProbe struct {
	identity SchemaIdentity
	err      error
}

func (p fixedSchemaProbe) Inspect(context.Context, string) (SchemaIdentity, error) {
	return p.identity, p.err
}

func TestReadinessAllowsNewWritableLayout(t *testing.T) {
	t.Parallel()

	layout, err := Initialize(filepath.Join(t.TempDir(), "librairii"))
	if err != nil {
		t.Fatalf("Initialize() error = %v", err)
	}
	checker := NewReadinessChecker(layout, fixedSchemaProbe{
		identity: SchemaIdentity{State: SchemaAbsent},
	})

	report, err := checker.Check(context.Background())
	if err != nil {
		t.Fatalf("Check() error = %v", err)
	}
	if !report.MutationsAllowed || len(report.Issues) != 0 {
		t.Fatalf("Check() report = %#v", report)
	}
}

func TestReadinessRejectsSchemaMismatch(t *testing.T) {
	t.Parallel()

	layout, err := Initialize(filepath.Join(t.TempDir(), "librairii"))
	if err != nil {
		t.Fatalf("Initialize() error = %v", err)
	}
	checker := NewReadinessChecker(layout, fixedSchemaProbe{
		identity: SchemaIdentity{State: SchemaForeign},
	})

	report, err := checker.Check(context.Background())
	if err != nil {
		t.Fatalf("Check() error = %v", err)
	}
	if report.MutationsAllowed || !hasIssue(report.Issues, IssueSchemaMismatch) {
		t.Fatalf("Check() report = %#v", report)
	}
}

func TestReadinessRejectsUnwritableArea(t *testing.T) {
	t.Parallel()

	layout, err := Initialize(filepath.Join(t.TempDir(), "librairii"))
	if err != nil {
		t.Fatalf("Initialize() error = %v", err)
	}
	checker := NewReadinessChecker(layout, fixedSchemaProbe{
		identity: SchemaIdentity{State: SchemaAbsent},
	})
	checker.probeWritable = func(directory string) error {
		if directory == layout.Archives {
			return os.ErrPermission
		}
		return nil
	}

	report, err := checker.Check(context.Background())
	if err != nil {
		t.Fatalf("Check() error = %v", err)
	}
	if report.MutationsAllowed || !hasIssue(report.Issues, IssueStorageNotWritable) {
		t.Fatalf("Check() report = %#v", report)
	}
}

func TestPathContainedResolvesSymlinkEscapes(t *testing.T) {
	t.Parallel()

	parent := t.TempDir()
	root := filepath.Join(parent, "root")
	outside := filepath.Join(parent, "outside")
	if err := os.MkdirAll(root, DirectoryMode); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(outside, DirectoryMode); err != nil {
		t.Fatal(err)
	}
	link := filepath.Join(root, "archives")
	if err := os.Symlink(outside, link); err != nil {
		t.Fatal(err)
	}

	contained, err := PathContained(root, link)
	if err != nil {
		t.Fatalf("PathContained() error = %v", err)
	}
	if contained {
		t.Fatal("PathContained() = true for symlink escape")
	}
}

func TestReadinessReturnsSchemaProbeFailure(t *testing.T) {
	t.Parallel()

	layout, err := Initialize(filepath.Join(t.TempDir(), "librairii"))
	if err != nil {
		t.Fatalf("Initialize() error = %v", err)
	}
	checker := NewReadinessChecker(layout, fixedSchemaProbe{err: errors.New("probe failed")})
	if _, err := checker.Check(context.Background()); err == nil {
		t.Fatal("Check() error = nil")
	}
}

func hasIssue(issues []ReadinessIssue, code ReadinessIssueCode) bool {
	for _, issue := range issues {
		if issue.Code == code {
			return true
		}
	}
	return false
}
