package main

import (
	"crypto/sha256"
	"fmt"
	"os"
	"strings"
	"testing"
)

const canonicalPrototypeSHA256 = "a3709c344b0c0dd0af362e0bdc7be3d8892560c2aec8175e3f3e48b87832bd16"
const canonicalPrototypeSnapshot = "testdata/ui-prototypes/05-archive-shelves.html"

func TestCanonicalPrototypeContract(t *testing.T) {
	t.Parallel()

	body, err := os.ReadFile(canonicalPrototypeSnapshot)
	if err != nil {
		t.Fatal(err)
	}
	if actual := fmt.Sprintf("%x", sha256.Sum256(body)); actual != canonicalPrototypeSHA256 {
		t.Fatalf(
			"canonical prototype SHA-256 = %s, want %s",
			actual,
			canonicalPrototypeSHA256,
		)
	}
	source := string(body)
	if !strings.Contains(source, `<div class="app">`) {
		t.Fatal("canonical prototype omitted normative .app subtree")
	}
	if !strings.Contains(source, `<a class="back"`) {
		t.Fatal("canonical prototype omitted gallery-only .back marker")
	}
}
