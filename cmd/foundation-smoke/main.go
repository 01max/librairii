package main

import (
	"context"
	"flag"
	"fmt"
	"os"
	"path/filepath"

	"github.com/01max/librairii/internal/database"
)

func main() {
	root := flag.String("root", "", "isolated Librairii application-data root")
	expectedStories := flag.Int(
		"expect-stories",
		-1,
		"expected persisted story count; disabled when negative",
	)
	expectedShelves := flag.Int(
		"expect-shelves",
		-1,
		"expected persisted shelf count; disabled when negative",
	)
	flag.Parse()

	if *root == "" || !filepath.IsAbs(*root) {
		fmt.Fprintln(os.Stderr, "-root must be an absolute path")
		os.Exit(2)
	}

	databasePath := filepath.Join(*root, "db", "librairii.sqlite3")
	identity, err := (database.SchemaProbe{}).Inspect(context.Background(), databasePath)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	if identity != database.IdentityCompatible {
		fmt.Fprintf(os.Stderr, "schema identity is %q, want %q\n", identity, database.IdentityCompatible)
		os.Exit(1)
	}
	connection, err := database.Open(context.Background(), databasePath)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	defer connection.Close()
	for _, expectation := range []struct {
		label string
		query string
		want  int
	}{
		{label: "stories", query: "SELECT COUNT(*) FROM stories", want: *expectedStories},
		{label: "shelves", query: "SELECT COUNT(*) FROM shelves", want: *expectedShelves},
	} {
		if expectation.want < 0 {
			continue
		}
		var count int
		if err := connection.SQL().QueryRowContext(
			context.Background(),
			expectation.query,
		).Scan(&count); err != nil {
			fmt.Fprintln(os.Stderr, err)
			os.Exit(1)
		}
		if count != expectation.want {
			fmt.Fprintf(
				os.Stderr,
				"%s count is %d, want %d\n",
				expectation.label,
				count,
				expectation.want,
			)
			os.Exit(1)
		}
	}

	fmt.Printf("foundation smoke verified %s\n", databasePath)
}
