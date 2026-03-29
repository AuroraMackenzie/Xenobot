-- Dedicated memory entries for persisted summaries, notes, and future recall layers.

CREATE TABLE IF NOT EXISTS memory_entry (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    meta_id INTEGER NOT NULL,
    chat_session_id INTEGER,
    memory_kind TEXT NOT NULL,
    title TEXT,
    content TEXT NOT NULL,
    tags TEXT NOT NULL DEFAULT '[]',
    source_label TEXT,
    importance INTEGER NOT NULL DEFAULT 50,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (meta_id) REFERENCES meta(id) ON DELETE CASCADE,
    FOREIGN KEY (chat_session_id) REFERENCES chat_session(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_memory_entry_kind_meta_chat_session
ON memory_entry(memory_kind, meta_id, chat_session_id);

CREATE INDEX IF NOT EXISTS idx_memory_entry_meta_updated
ON memory_entry(meta_id, updated_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_memory_entry_kind_meta_updated
ON memory_entry(memory_kind, meta_id, updated_at DESC, id DESC);
