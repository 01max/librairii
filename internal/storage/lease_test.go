//go:build darwin || linux

package storage

import (
	"bufio"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"
)

func TestLeaseExcludesAliasesAndRetainsItsFileAfterClose(t *testing.T) {
	root := t.TempDir()
	first, err := AcquireLibrary(root)
	if err != nil {
		t.Fatal(err)
	}
	alias := filepath.Join(t.TempDir(), "alias")
	if err := os.Symlink(root, alias); err != nil {
		t.Fatal(err)
	}
	for _, path := range []string{root, alias} {
		if other, err := AcquireLibrary(path); !errors.Is(err, ErrLibraryInUse) {
			if other != nil {
				other.Close()
			}
			t.Fatalf("second owner at %s: %v", path, err)
		}
	}
	before, err := os.Stat(filepath.Join(root, LibraryLockFile))
	if err != nil {
		t.Fatal(err)
	}
	if err := first.Close(); err != nil {
		t.Fatal(err)
	}
	second, err := AcquireLibrary(root)
	if err != nil {
		t.Fatal(err)
	}
	defer second.Close()
	after, err := os.Stat(filepath.Join(root, LibraryLockFile))
	if err != nil || !os.SameFile(before, after) {
		t.Fatalf("unlock replaced the lock file: %v", err)
	}
}

func TestLeaseRefusesLinksAndDetectsNameReplacement(t *testing.T) {
	root := t.TempDir()
	leaf := filepath.Join(root, LibraryLockFile)
	foreign := filepath.Join(t.TempDir(), "foreign")
	if err := os.WriteFile(foreign, []byte("preserved"), 0600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(foreign, leaf); err != nil {
		t.Fatal(err)
	}
	if owner, err := AcquireLibrary(root); err == nil {
		owner.Close()
		t.Fatal("followed foreign lock symlink")
	}
	if err := os.Remove(leaf); err != nil {
		t.Fatal(err)
	}
	if err := os.Link(foreign, leaf); err != nil {
		t.Fatal(err)
	}
	if owner, err := AcquireLibrary(root); !errors.Is(err, ErrLeaseChanged) {
		if owner != nil {
			owner.Close()
		}
		t.Fatal(err)
	}
	if err := os.Remove(leaf); err != nil {
		t.Fatal(err)
	}
	owner, err := AcquireLibrary(root)
	if err != nil {
		t.Fatal(err)
	}
	defer owner.Close()
	if err := os.Rename(leaf, leaf+".retained"); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(leaf, nil, 0600); err != nil {
		t.Fatal(err)
	}
	if err := owner.Revalidate(); !errors.Is(err, ErrLeaseChanged) {
		t.Fatal(err)
	}
	bytes, err := os.ReadFile(foreign)
	if err != nil || string(bytes) != "preserved" {
		t.Fatalf("foreign lock changed: %q %v", bytes, err)
	}
}

func TestLeaseOwnerProcessDeathReleasesKernelOwnership(t *testing.T) {
	root := t.TempDir()
	self, err := os.Executable()
	if err != nil {
		t.Fatal(err)
	}
	child := exec.Command(self, "-test.run=^TestLeaseChild$")
	child.Env = append(os.Environ(), "LIBRAIRII_LEASE_TEST_CHILD="+root)
	input, err := child.StdinPipe()
	if err != nil {
		t.Fatal(err)
	}
	output, err := child.StdoutPipe()
	if err != nil {
		t.Fatal(err)
	}
	child.Stderr = os.Stderr
	if err := child.Start(); err != nil {
		t.Fatal(err)
	}
	defer func() { input.Close(); child.Process.Kill(); child.Wait() }()
	ready := make(chan string, 1)
	go func() { line, _ := bufio.NewReader(output).ReadString('\n'); ready <- line }()
	select {
	case line := <-ready:
		if line != "owned\n" {
			t.Fatal(line)
		}
	case <-time.After(10 * time.Second):
		t.Fatal("child did not acquire lock")
	}
	if owner, err := AcquireLibrary(root); !errors.Is(err, ErrLibraryInUse) {
		if owner != nil {
			owner.Close()
		}
		t.Fatal(err)
	}
	if err := child.Process.Kill(); err != nil {
		t.Fatal(err)
	}
	if err := child.Wait(); err == nil {
		t.Fatal("child was not forcibly terminated")
	}
	owner, err := AcquireLibrary(root)
	if err != nil {
		t.Fatal(err)
	}
	defer owner.Close()
}

func TestLeaseChild(t *testing.T) {
	root := os.Getenv("LIBRAIRII_LEASE_TEST_CHILD")
	if root == "" {
		return
	}
	owner, err := AcquireLibrary(root)
	if err != nil {
		t.Fatal(err)
	}
	defer owner.Close()
	fmt.Println("owned")
	if _, err := io.Copy(io.Discard, os.Stdin); err != nil {
		t.Fatal(err)
	}
}
