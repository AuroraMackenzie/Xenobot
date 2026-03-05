-- Query hot-path indexes for chat/API analytics and pagination.
-- Covers common filter/sort paths used by repository + chat handlers.

-- Session-scoped timeline scans and pagination:
-- WHERE meta_id = ? ORDER BY ts DESC, id DESC LIMIT/OFFSET
CREATE INDEX IF NOT EXISTS idx_message_meta_ts_id
ON message(meta_id, ts DESC, id DESC);

-- Member-focused timeline scans:
-- WHERE meta_id = ? AND sender_id = ? ORDER BY ts DESC, id DESC
CREATE INDEX IF NOT EXISTS idx_message_meta_sender_ts_id
ON message(meta_id, sender_id, ts DESC, id DESC);

-- Session listing by meta:
-- WHERE meta_id = ? ORDER BY created_at DESC
CREATE INDEX IF NOT EXISTS idx_sessions_meta_created_at
ON sessions(meta_id, created_at DESC);

-- Name history lookups:
-- WHERE member_id = ? ORDER BY start_ts DESC
CREATE INDEX IF NOT EXISTS idx_member_name_history_member_start_ts
ON member_name_history(member_id, start_ts DESC);

-- Message context timeline joins:
-- WHERE session_id = ? ORDER BY message_id
CREATE INDEX IF NOT EXISTS idx_message_context_session_message
ON message_context(session_id, message_id);

-- Chat-session ranges per chat:
-- WHERE meta_id = ? ORDER BY start_ts DESC, id DESC
CREATE INDEX IF NOT EXISTS idx_chat_session_meta_start_ts_id
ON chat_session(meta_id, start_ts DESC, id DESC);
