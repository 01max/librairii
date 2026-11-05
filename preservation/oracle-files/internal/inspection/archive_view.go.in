package inspection

import "context"

type archiveView interface {
	has(string) bool
	hasFileBelow(string) bool
	entryNames() []string
	read(context.Context, string, int64, ErrorCode) ([]byte, error)
}
