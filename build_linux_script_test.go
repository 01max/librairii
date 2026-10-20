package main

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func TestLinuxBuildDispatchAndFailures(t *testing.T) {
	body, err := os.ReadFile("scripts/build-linux")
	if err != nil {
		t.Fatal(err)
	}
	for _, failure := range []string{"", "engine", "buildx", "build"} {
		name := failure
		if name == "" {
			name = "success"
		}
		t.Run(name, func(t *testing.T) {
			t.Parallel()
			root := filepath.Join(t.TempDir(), "checkout with spaces")
			bin := filepath.Join(root, "bin")
			for _, dir := range []string{bin, filepath.Join(root, "scripts")} {
				if err := os.MkdirAll(dir, 0o700); err != nil {
					t.Fatal(err)
				}
			}
			files := map[string]string{
				"scripts/build-linux": string(body),
				".nvmrc":              "24.15.0\n",
				".wails-version":      "v2.16.0\n",
				"VERSION":             "0.5.0\n",
				"bin/docker": `#!/bin/sh
set -eu
case "$*" in
    info) [ "$LINUX_TEST_FAILURE" != engine ]; exit ;;
    'buildx version') [ "$LINUX_TEST_FAILURE" != buildx ]; exit ;;
esac
pwd > "$LINUX_TEST_DIRECTORY"
printf '%s\n' "$@" > "$LINUX_TEST_ARGUMENTS"
[ "$LINUX_TEST_FAILURE" != build ]
`,
			}
			for path, content := range files {
				if err := os.WriteFile(filepath.Join(root, path), []byte(content), 0o700); err != nil {
					t.Fatal(err)
				}
			}
			arguments := filepath.Join(root, "arguments")
			directory := filepath.Join(root, "directory")
			cmd := exec.Command("sh", filepath.Join(root, "scripts/build-linux"))
			cmd.Dir = t.TempDir() // Invocation need not start at the checkout root.
			cmd.Env = append(os.Environ(),
				"PATH="+bin+string(os.PathListSeparator)+os.Getenv("PATH"),
				"LINUX_TEST_FAILURE="+failure,
				"LINUX_TEST_ARGUMENTS="+arguments,
				"LINUX_TEST_DIRECTORY="+directory,
			)
			output, err := cmd.CombinedOutput()
			if (err == nil) != (failure == "") {
				t.Fatalf("build result = %v, output:\n%s", err, output)
			}
			if failure != "" {
				if strings.Contains(string(output), "Linux archive:") {
					t.Fatalf("failed build claimed success:\n%s", output)
				}
				if failure != "build" {
					if _, err := os.Stat(arguments); !os.IsNotExist(err) {
						t.Fatalf("build invoked after failed prerequisite: %v", err)
					}
				}
				return
			}
			args, err := os.ReadFile(arguments)
			if err != nil {
				t.Fatal(err)
			}
			want := "buildx\nbuild\n--platform\nlinux/amd64\n--file\nbuild/linux/Dockerfile\n" +
				"--build-arg\nNODE_VERSION=24.15.0\n--build-arg\nWAILS_VERSION=v2.16.0\n" +
				"--output\ntype=local,dest=build/bin\n.\n"
			if string(args) != want {
				t.Fatalf("Docker arguments:\n%s\nwant:\n%s", args, want)
			}
			cwd, err := os.ReadFile(directory)
			if err != nil || strings.TrimSpace(string(cwd)) != root {
				t.Fatalf("Docker directory = %q, want %q (%v)", cwd, root, err)
			}
			if !strings.Contains(string(output), "Librairii-0.5.0-linux-amd64.tar.gz.sha256") {
				t.Fatalf("missing versioned output paths:\n%s", output)
			}
		})
	}
}
