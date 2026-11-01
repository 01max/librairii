CREATE UNIQUE INDEX idx_file_operations_one_active_metadata_sync
    ON file_operations (kind)
    WHERE (
        kind = 'metadata_sync'
        AND status IN ('queued', 'running')
    );
