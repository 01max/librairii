package tagging

import (
	"context"
	"database/sql"
	"github.com/01max/librairii/internal/database"
	"os"
	"path/filepath"
	"testing"
)

func openTaggingDatabase(t *testing.T) *sql.DB {
	t.Helper()

	path := filepath.Join(t.TempDir(), "db", "librairii.sqlite3")
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		t.Fatal(err)
	}
	sqlDatabase, err := database.Open(context.Background(), path)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := sqlDatabase.Close(); err != nil {
			t.Error(err)
		}
	})
	return sqlDatabase.SQL()
}

func seedTestBoolean(ctx context.Context, connection *sql.DB) (Definition, error) {
	service, err := NewService(connection)
	if err != nil {
		return Definition{}, err
	}
	return service.CreateDefinition(ctx, CreateDefinition{Key: "fixture-boolean", Label: "Favorite", Color: "#405CF5", Kind: KindBoolean})
}
