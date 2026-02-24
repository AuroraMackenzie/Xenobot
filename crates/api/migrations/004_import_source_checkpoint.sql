-- Persistent checkpoint table for legal-safe incremental imports/monitoring.
-- Keeps per-source fingerprint and last write summary to skip unchanged exports.
CREATE TABLE IF NOT EXISTS import_source_checkpoint (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_kind TEXT NOT NULL,
    source_path TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    file_size INTEGER NOT NULL DEFAULT 0,
    modified_at INTEGER NOT NULL DEFAULT 0,
    platform TEXT,
    chat_name TEXT,
    meta_id INTEGER,
    last_processed_at INTEGER NOT NULL,
    last_inserted_messages INTEGER NOT NULL DEFAULT 0,
    last_duplicate_messages INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'completed',
    error_message TEXT,
    UNIQUE(source_kind, source_path),
    FOREIGN KEY (meta_id) REFERENCES meta(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_import_source_checkpoint_kind_status
ON import_source_checkpoint(source_kind, status);

CREATE INDEX IF NOT EXISTS idx_import_source_checkpoint_processed_at
ON import_source_checkpoint(last_processed_at);
