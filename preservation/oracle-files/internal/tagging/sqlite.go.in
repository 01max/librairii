package tagging

import (
	"errors"

	sqliteDriver "modernc.org/sqlite"
	sqlite3 "modernc.org/sqlite/lib"
)

func isUniqueConstraint(err error) bool {
	var sqliteError *sqliteDriver.Error
	return errors.As(err, &sqliteError) &&
		sqliteError.Code() == sqlite3.SQLITE_CONSTRAINT_UNIQUE
}
