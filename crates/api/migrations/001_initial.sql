CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS meta (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    platform TEXT NOT NULL,
    chat_type TEXT NOT NULL,
    imported_at INTEGER NOT NULL,
    group_id TEXT,
    group_avatar TEXT,
    owner_id TEXT,
    schema_version INTEGER DEFAULT 3,
    session_gap_threshold INTEGER DEFAULT 1800
);

CREATE TABLE IF NOT EXISTS member (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    platform_id TEXT NOT NULL UNIQUE,
    account_name TEXT,
    group_nickname TEXT,
    aliases TEXT DEFAULT '[]',
    avatar TEXT,
    roles TEXT DEFAULT '[]'
);

CREATE TABLE IF NOT EXISTS member_name_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    member_id INTEGER NOT NULL,
    name_type TEXT NOT NULL,
    name TEXT NOT NULL,
    start_ts INTEGER NOT NULL,
    end_ts INTEGER,
    FOREIGN KEY (member_id) REFERENCES member(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS message (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sender_id INTEGER NOT NULL,
    sender_account_name TEXT,
    sender_group_nickname TEXT,
    ts INTEGER NOT NULL,
    msg_type INTEGER NOT NULL,
    content TEXT,
    reply_to_message_id TEXT DEFAULT NULL,
    platform_message_id TEXT DEFAULT NULL,
    meta_id INTEGER NOT NULL,
    FOREIGN KEY (sender_id) REFERENCES member(id) ON DELETE CASCADE,
    FOREIGN KEY (meta_id) REFERENCES meta(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_message_ts ON message(ts);
CREATE INDEX IF NOT EXISTS idx_message_sender ON message(sender_id);
CREATE INDEX IF NOT EXISTS idx_message_meta ON message(meta_id);

CREATE TABLE IF NOT EXISTS chat_session (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    meta_id INTEGER NOT NULL,
    start_ts INTEGER NOT NULL,
    end_ts INTEGER NOT NULL,
    message_count INTEGER DEFAULT 0,
    is_manual INTEGER DEFAULT 0,
    summary TEXT,
    FOREIGN KEY (meta_id) REFERENCES meta(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_chat_session_meta ON chat_session(meta_id);

CREATE TABLE IF NOT EXISTS message_context (
    message_id INTEGER PRIMARY KEY,
    session_id INTEGER NOT NULL,
    topic_id INTEGER,
    FOREIGN KEY (message_id) REFERENCES message(id) ON DELETE CASCADE,
    FOREIGN KEY (session_id) REFERENCES chat_session(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS embedding_cache (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    message_id INTEGER NOT NULL,
    content TEXT NOT NULL,
    embedding BLOB NOT NULL,
    model TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (message_id) REFERENCES message(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_embedding_message ON embedding_cache(message_id);

CREATE TABLE IF NOT EXISTS analysis_cache (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    meta_id INTEGER NOT NULL,
    analysis_type TEXT NOT NULL,
    result TEXT NOT NULL,
    params TEXT,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (meta_id) REFERENCES meta(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_analysis_meta_type ON analysis_cache(meta_id, analysis_type);

CREATE TABLE IF NOT EXISTS sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    meta_id INTEGER NOT NULL,
    title TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (meta_id) REFERENCES meta(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS session_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER NOT NULL,
    message_id INTEGER NOT NULL,
    order_index INTEGER NOT NULL,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (message_id) REFERENCES message(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_session_messages_session ON session_messages(session_id);

CREATE TABLE IF NOT EXISTS import_progress (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path TEXT NOT NULL,
    total_messages INTEGER DEFAULT 0,
    processed_messages INTEGER DEFAULT 0,
    status TEXT DEFAULT 'pending',
    started_at INTEGER,
    completed_at INTEGER,
    error_message TEXT
);

CREATE TABLE IF NOT EXISTS conversations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    messages TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_conversations_session ON conversations(session_id);