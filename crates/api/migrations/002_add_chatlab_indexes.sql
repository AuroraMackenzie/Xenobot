CREATE INDEX IF NOT EXISTS idx_message_platform_id ON message(platform_message_id);

CREATE INDEX IF NOT EXISTS idx_member_name_history_member_id ON member_name_history(member_id);

CREATE INDEX IF NOT EXISTS idx_chat_session_time ON chat_session(start_ts, end_ts);

CREATE INDEX IF NOT EXISTS idx_message_context_session ON message_context(session_id);

CREATE INDEX IF NOT EXISTS idx_message_sender_ts ON message(sender_id, ts);

CREATE INDEX IF NOT EXISTS idx_meta_platform_type ON meta(platform, chat_type);

CREATE INDEX IF NOT EXISTS idx_member_platform_id ON member(platform_id);

CREATE INDEX IF NOT EXISTS idx_import_progress_status ON import_progress(status);