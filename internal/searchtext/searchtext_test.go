package searchtext

import "testing"

func TestNormalizeFoldsCaseAccentsAndWhitespaceButKeepsLiteralWildcards(t *testing.T) {
	t.Parallel()

	got := Normalize("  ÉTÉ\t100%_Magique  ")
	if got != "ete 100%_magique" {
		t.Fatalf("Normalize() = %q", got)
	}
}
