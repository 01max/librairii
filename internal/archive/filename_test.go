package archive

import "testing"

func TestValidFilenameRejectsNormalizationAndPathChanges(t *testing.T) {
	t.Parallel()

	for _, test := range []struct {
		name  string
		valid bool
	}{
		{name: "Clockwork Forest.v2.pk", valid: true},
		{name: " clockwork.zip"},
		{name: "clockwork.zip "},
		{name: "../clockwork.zip"},
		{name: `folder\clockwork.zip`},
		{name: "."},
		{name: ".."},
		{name: "clockwork\x00.zip"},
	} {
		if got := ValidFilename(test.name); got != test.valid {
			t.Errorf("ValidFilename(%q) = %t, want %t", test.name, got, test.valid)
		}
	}
}
