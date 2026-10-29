package handover

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

const testLibrary = "f1000000-0000-4000-8000-000000000017"

func TestLibraryBoundExportWhitelistsStateAndDoesNotClobber(t *testing.T) {
	state := State{Presentation: "small_covers", SyncPanelHidden: true, DismissedOperations: []string{"f2000000-0000-4000-8000-000000000017"}, Frames: []Frame{{Kind: "custom", Hash: "#/library?v=3&name=bonjour%20%C3%A9&page=3&sort=imported_desc"}}}
	body, err := Encode(testLibrary, 3, state)
	if err != nil {
		t.Fatal(err)
	}
	var document Document
	if err := json.Unmarshal(body, &document); err != nil {
		t.Fatal(err)
	}
	if document.LibraryID != testLibrary || document.Version != 1 || document.State.Frames[0].Hash != state.Frames[0].Hash {
		t.Fatal(document)
	}
	upper, err := Encode(strings.ToUpper(testLibrary), 3, state)
	if err != nil || string(upper) != string(body) {
		t.Fatalf("valid legacy UUID spelling lost binding: %v", err)
	}
	directory := t.TempDir()
	if err := Export(directory, body); err != nil {
		t.Fatal(err)
	}
	if err := Export(directory, []byte("replacement")); err == nil {
		t.Fatal("overwrote existing transfer")
	}
	actual, err := os.ReadFile(filepath.Join(directory, Filename))
	if err != nil || string(actual) != string(body) {
		t.Fatalf("transfer changed: %v", err)
	}
	entries, err := os.ReadDir(directory)
	if err != nil || len(entries) != 1 {
		t.Fatalf("temporary file remained: %v %v", entries, err)
	}
}

func TestHandoverRejectsUnboundUnboundedAndAmbiguousState(t *testing.T) {
	valid := `{"presentation":"list","dismissedOperations":[],"syncPanelHidden":false,"frames":[{"hash":"","kind":"library"}],"position":0}`
	for _, input := range []string{
		strings.Replace(valid, `"position":0`, `"position":0,"position":1`, 1),
		strings.Replace(valid, `"position":0`, `"position":1`, 1),
		strings.Replace(valid, `"kind":"library"`, `"kind":"collection"`, 1),
		strings.Replace(valid, `"hash":""`, `"hash":"#/library?v=3&token=secret"`, 1),
		strings.Replace(valid, `"position":0`, `"position":0,"browserStorage":"secret"`, 1),
		strings.Repeat(" ", MaxBytes) + valid,
	} {
		if _, err := DecodeState(input); err == nil {
			t.Fatalf("invalid transfer admitted: %.100s", input)
		}
	}
	state, err := DecodeState(valid)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := Encode("not-a-library", 0, state); err == nil {
		t.Fatal("unbound export admitted")
	}
	state.Frames = make([]Frame, 101)
	if _, err := Encode(testLibrary, 0, state); err == nil {
		t.Fatal("unbounded history admitted")
	}
}

func TestSharedGoRustFixtureIsExactBridgeOutput(t *testing.T) {
	body, err := os.ReadFile("../../crates/librairii-core/tests/fixtures/handover-v1.json")
	if err != nil {
		t.Fatal(err)
	}
	var document Document
	if err := json.Unmarshal(body, &document); err != nil {
		t.Fatal(err)
	}
	actual, err := Encode(document.LibraryID, document.LegacyBrokenTagID, document.State)
	if err != nil || string(actual) != string(body) {
		t.Fatalf("shared fixture no longer matches Go encoder: %v", err)
	}
}
