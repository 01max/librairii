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

	fmt.Printf("foundation smoke verified %s\n", databasePath)
}
