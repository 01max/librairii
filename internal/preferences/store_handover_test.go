package preferences

import "testing"

func TestLanguageUpdateRefreshesAuthorityCommittedByAnotherStore(t *testing.T) {
	directory := t.TempDir()
	owner, stale := NewStore(directory), NewStore(directory)
	if _, err := owner.Update(func(p *Profile) error { p.Root = "/old"; p.LibraryID = "identity"; p.Complete = true; return nil }); err != nil {
		t.Fatal(err)
	}
	if _, err := stale.Load(); err != nil {
		t.Fatal(err)
	}
	if _, err := owner.Update(func(p *Profile) error { p.Root = "/committed-new-root"; return nil }); err != nil {
		t.Fatal(err)
	}
	if _, err := stale.Update(func(p *Profile) error { p.Language = "fr"; return nil }); err != nil {
		t.Fatal(err)
	}
	actual, err := NewStore(directory).Load()
	if err != nil || actual.Root != "/committed-new-root" || actual.Language != "fr" || actual.LibraryID != "identity" {
		t.Fatalf("stale language reset library authority: %+v %v", actual, err)
	}
}
