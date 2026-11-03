package database

import (
	"context"
	"database/sql"
	"errors"
	"io"
	"os"
	"strings"

	_ "modernc.org/sqlite"
)

type Identity string

const (
	IdentityAbsent     Identity = "absent"
	IdentityCompatible Identity = "compatible"
	IdentityForeign    Identity = "foreign"
)

type SchemaProbe struct{}

func (SchemaProbe) Inspect(ctx context.Context, path string) (Identity, error) {
	file, err := os.Open(path)
	if errors.Is(err, os.ErrNotExist) {
		return IdentityAbsent, nil
	}
	if err != nil {
		return "", err
	}

	header := make([]byte, 16)
	_, readErr := io.ReadFull(file, header)
	closeErr := file.Close()
	if readErr != nil || closeErr != nil || string(header) != "SQLite format 3\x00" {
		return IdentityForeign, nil
	}

	connection, err := sql.Open("sqlite", readOnlyDSN(path))
	if err != nil {
		return "", err
	}
	defer connection.Close()

	var product string
	var family string
	err = connection.QueryRowContext(
		ctx,
		"SELECT product, schema_family FROM librairii_schema_identity WHERE singleton = 1",
	).Scan(&product, &family)
	if err != nil {
		if errors.Is(err, sql.ErrNoRows) || strings.Contains(err.Error(), "no such table") {
			return IdentityForeign, nil
		}
		return "", err
	}
	if product != SchemaProduct || family != SchemaFamily {
		return IdentityForeign, nil
	}
	return IdentityCompatible, nil
}
