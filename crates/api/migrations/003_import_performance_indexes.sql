-- Import/incremental write performance indexes.
-- Optimizes dedup lookup: message_exists(meta_id, sender_id, ts, msg_type, content)
CREATE INDEX IF NOT EXISTS idx_message_dedup_lookup
ON message(meta_id, sender_id, ts, msg_type, content);

-- Optimizes incremental chat/session target lookup by platform and chat name.
CREATE INDEX IF NOT EXISTS idx_meta_platform_name
ON meta(platform, name);

