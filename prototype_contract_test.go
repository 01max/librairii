package main

import (
	"crypto/sha256"
	"fmt"
	"os"
	"strings"
	"testing"
)

const canonicalPrototypeSHA256 = "d519dd99e6e47dcdf8bead73e42dea2c43bb26b57f00f66d906375fc3a861860"
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
