package shelves

import (
	"errors"
	"strings"

	sqliteDriver "modernc.org/sqlite"
	sqlite3 "modernc.org/sqlite/lib"
)

func isDuplicateShelfName(err error) bool {
	var sqliteError *sqliteDriver.Error
	return errors.As(err, &sqliteError) &&
		sqliteError.Code() == sqlite3.SQLITE_CONSTRAINT_UNIQUE &&
		strings.Contains(sqliteError.Error(), "shelves.normalized_name")
}
