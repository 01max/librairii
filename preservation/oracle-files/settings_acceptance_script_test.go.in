package main

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestSettingsAcceptanceCleanup(t *testing.T) {
	for _, tc := range []struct {
		name          string
		failedRemoves int
		appStatus     int
		wantSuccess   bool
		wantAttempts  int
	}{
		{"immediate cleanup", 0, 0, true, 1},
		{"cache writer races cleanup", 1, 0, true, 2},
		{"persistent cleanup failure", 10, 0, false, 5},
		{"acceptance failure survives cleanup retry", 1, 1, false, 2},
	} {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			root := t.TempDir()
			bin := filepath.Join(root, "bin")
			if err := os.Mkdir(bin, 0o700); err != nil {
				t.Fatal(err)
			}
			writeScript := func(name, body string) string {
				t.Helper()
				path := filepath.Join(bin, name)
				if err := os.WriteFile(path, []byte("#!/bin/sh\nset -eu\n"+body), 0o700); err != nil {
					t.Fatal(err)
				}
				return path
			}
			// Avoid requiring a Linux desktop: the script's profile, verification,
			// watchdog and EXIT trap all run, with a fake application and rm.
			writeScript("uname", "printf 'Darwin\\n'\n")
			writeScript("sleep", "if [ \"$1\" = 45 ]; then exec /bin/sleep 1 >/dev/null 2>&1; fi\nexec /bin/sleep \"$@\"\n")
			writeScript("rm", `
count=0
if [ -f "$ACCEPTANCE_TEST_COUNT" ]; then read -r count < "$ACCEPTANCE_TEST_COUNT"; fi
count=$((count + 1))
printf '%s\n' "$count" > "$ACCEPTANCE_TEST_COUNT"
if [ "$count" -le "$ACCEPTANCE_TEST_FAILED_REMOVES" ]; then
    printf 'rm: cannot remove temporary profile cache: Directory not empty\n' >&2
    exit 1
fi
exec /bin/rm "$@"
`)
			application := writeScript("application", `
if [ "$ACCEPTANCE_TEST_APP_STATUS" -ne 0 ]; then exit "$ACCEPTANCE_TEST_APP_STATUS"; fi
printf '%s\n' \
    scenario_started native_library_dialog_selected setup_completed settings_menu_french_selected settings_menu_selected \
    native_import_dialog_selected import_queued import_succeeded collection_loaded \
    native_relocation_dialog_selected library_relocated \
    native_destination_dialog_selected export_prepared export_queued export_succeeded \
    native_reveal_succeeded reveal_succeeded complete > "$LIBRAIRII_ACCEPTANCE_CHECKPOINTS"
mkdir -p "$LIBRAIRII_ACCEPTANCE_RELOCATION/db"
touch "$LIBRAIRII_ACCEPTANCE_RELOCATION/db/librairii.sqlite3"
/bin/rm -rf "$LIBRAIRII_ACCEPTANCE_LIBRARY"
`)
			countPath := filepath.Join(root, "removes")
			ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
			defer cancel()
			cmd := exec.CommandContext(ctx, "sh", "scripts/verify-settings-acceptance", application)
			cmd.Env = append(os.Environ(),
				"PATH="+bin+string(os.PathListSeparator)+os.Getenv("PATH"),
				"TMPDIR="+root,
				"ACCEPTANCE_TEST_COUNT="+countPath,
				fmt.Sprintf("ACCEPTANCE_TEST_FAILED_REMOVES=%d", tc.failedRemoves),
				fmt.Sprintf("ACCEPTANCE_TEST_APP_STATUS=%d", tc.appStatus),
			)
			output, err := cmd.CombinedOutput()
			if (err == nil) != tc.wantSuccess {
				t.Fatalf("acceptance result = %v, want success %t:\n%s", err, tc.wantSuccess, output)
			}
			attempts, err := os.ReadFile(countPath)
			if err != nil {
				t.Fatal(err)
			}
			if strings.TrimSpace(string(attempts)) != fmt.Sprint(tc.wantAttempts) {
				t.Fatalf("cleanup attempts = %s, want %d:\n%s", attempts, tc.wantAttempts, output)
			}
			if tc.failedRemoves < 5 {
				remaining, err := filepath.Glob(filepath.Join(root, "librairii-settings-acceptance.*"))
				if err != nil || len(remaining) != 0 {
					t.Fatalf("temporary profiles remain: %v (%v)", remaining, err)
				}
			}
		})
	}
}
