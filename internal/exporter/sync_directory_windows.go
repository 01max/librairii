//go:build windows

package exporter

// Windows publication uses MOVEFILE_WRITE_THROUGH. Flushing a directory
// handle is unsupported and would turn a durable successful move into a
// reported failure.
func syncExportDirectory(string) error {
	return nil
}
