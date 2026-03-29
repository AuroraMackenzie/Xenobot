use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;
use tracing::{info, warn};

/// Errors that can occur during chat parsing.
#[derive(Error, Debug)]
pub enum ParseError {
    /// IO operation error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// JSON parsing error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    /// Unsupported chat format.
    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),
    /// General parse error.
    #[error("Parse error: {0}")]
    Parse(String),
    /// Invalid format specification.
    #[error("Invalid format: {0}")]
    InvalidFormat(String),
}

/// A parsed message from a chat export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedMessage {
    /// Unique sender identifier.
    pub sender: String,
    /// Optional display name of the sender.
    pub sender_name: Option<String>,
    /// Unix timestamp of the message.
    pub timestamp: i64,
    /// Message content.
    pub content: String,
    /// Type of message.
    pub msg_type: MessageType,
}

/// Type of chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    /// Plain text message.
    Text,
    /// Image media.
    Image,
    /// Video media.
    Video,
    /// Audio media.
    Audio,
    /// File attachment.
    File,
    /// Sticker or emoji.
    Sticker,
    /// Location sharing.
    Location,
    /// System message (join/leave/etc).
    System,
    /// Link preview.
    Link,
}

/// A complete parsed chat session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedChat {
    /// Platform name (e.g., "whatsapp", "telegram").
    pub platform: String,
    /// Name of the chat/group.
    pub chat_name: String,
    /// Type of chat.
    pub chat_type: ChatType,
    /// List of messages in the chat.
    pub messages: Vec<ParsedMessage>,
    /// List of members in the chat.
    pub members: Vec<ChatMember>,
}

/// Type of chat (private or group).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChatType {
    /// One-on-one private chat.
    Private,
    /// Group chat.
    Group,
}

/// A member of a chat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMember {
    /// Unique member identifier.
    pub id: String,
    /// Optional member name.
    pub name: Option<String>,
    /// Optional display name.
    pub display_name: Option<String>,
}

/// Trait for platform-specific chat parsers.
pub trait ChatParser: Send + Sync {
    /// Returns the name of the parser.
    fn name(&self) -> &str;
    /// Check if this parser can handle the given file.
    fn can_parse(&self, path: &Path) -> bool;
    /// Parse the chat file and return structured data.
    fn parse(&self, path: &Path) -> Result<ParsedChat, ParseError>;
}

/// Registry of available chat parsers.
pub struct ParserRegistry {
    parsers: Vec<Box<dyn ChatParser>>,
}

impl ParserRegistry {
    /// Creates a new registry with default parsers.
    pub fn new() -> Self {
        let mut registry = Self {
            parsers: Vec::new(),
        };
        registry.register_default_parsers();
        registry
    }

    fn register_default_parsers(&mut self) {
        self.parsers.push(Box::new(ManualReviewParser::new()));
        self.parsers.push(Box::new(WhatsAppParser::new()));
        self.parsers.push(Box::new(LINEParser::new()));
        self.parsers.push(Box::new(QQParser::new()));
        self.parsers.push(Box::new(TelegramParser::new()));
        self.parsers.push(Box::new(DiscordParser::new()));
        self.parsers.push(Box::new(WeChatParser::new()));
        self.parsers.push(Box::new(InstagramParser::new()));
        self.parsers.push(Box::new(IMessageParser::new()));
        self.parsers.push(Box::new(MessengerParser::new()));
        self.parsers.push(Box::new(KakaoTalkParser::new()));
        self.parsers.push(Box::new(SlackParser::new()));
        self.parsers.push(Box::new(TeamsParser::new()));
        self.parsers.push(Box::new(SignalParser::new()));
        self.parsers.push(Box::new(SkypeParser::new()));
        self.parsers.push(Box::new(GoogleChatParser::new()));
        self.parsers.push(Box::new(ZoomParser::new()));
        self.parsers.push(Box::new(ViberParser::new()));
    }

    /// Register a new parser.
    ///
    /// Adds a custom parser to the registry.
    pub fn register(&mut self, parser: Box<dyn ChatParser>) {
        self.parsers.push(parser);
    }

    /// Detect the chat format and parse the file.
    ///
    /// Tries each registered parser in order until one successfully parses the file.
    pub fn detect_and_parse(&self, path: &Path) -> Result<ParsedChat, ParseError> {
        let path_lower = path.to_string_lossy().to_lowercase();
        let mut best_match: Option<(usize, ParsedChat, String)> = None;
        let mut hinted_empty_fallback: Option<(ParsedChat, String)> = None;
        let mut last_hinted_error: Option<ParseError> = None;
        let mut saw_hinted_parser = false;
        let mut attempted = vec![false; self.parsers.len()];

        // Pass 1: respect parser-level hints for fast-path matching.
        // Pass 2: broaden to all parsers only if pass 1 did not produce a confident match.
        for pass in 0..=1 {
            for (idx, parser) in self.parsers.iter().enumerate() {
                let hinted = parser.can_parse(path);
                if pass == 0 && !hinted {
                    continue;
                }
                if attempted[idx] {
                    continue;
                }
                attempted[idx] = true;
                if hinted {
                    saw_hinted_parser = true;
                }

                match parser.parse(path) {
                    Ok(parsed) => {
                        let score = score_parsed_chat(&parsed, parser.name(), &path_lower, hinted);
                        if score > 0 {
                            let should_replace = best_match
                                .as_ref()
                                .map(|(best_score, _, _)| score > *best_score)
                                .unwrap_or(true);
                            if should_replace {
                                best_match = Some((score, parsed, parser.name().to_string()));
                            }
                        } else if hinted && hinted_empty_fallback.is_none() {
                            // Keep a deterministic fallback only for hinted parsers.
                            hinted_empty_fallback = Some((parsed, parser.name().to_string()));
                        }
                    }
                    Err(error) => {
                        if hinted {
                            last_hinted_error = Some(error);
                        }
                    }
                }
            }

            if best_match.is_some() {
                break;
            }
        }

        if let Some((_, parsed, parser_name)) = best_match {
            info!("Detected format: {}", parser_name);
            return Ok(parsed);
        }

        if let Some((parsed, parser_name)) = hinted_empty_fallback {
            warn!(
                "Parser '{}' returned empty message set; accepting hinted fallback",
                parser_name
            );
            return Ok(parsed);
        }

        if saw_hinted_parser {
            if let Some(error) = last_hinted_error {
                return Err(error);
            }
        }

        Err(ParseError::UnsupportedFormat(
            "Unknown chat format".to_string(),
        ))
    }

    /// Returns all registered parser names.
    pub fn parser_names(&self) -> Vec<String> {
        self.parsers
            .iter()
            .map(|parser| parser.name().to_string())
            .collect()
    }

    /// Returns number of registered parsers.
    pub fn parser_count(&self) -> usize {
        self.parsers.len()
    }
}

impl Default for ParserRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn score_parsed_chat(
    parsed: &ParsedChat,
    parser_name: &str,
    path_lower: &str,
    hinted: bool,
) -> usize {
    if parsed.messages.is_empty() {
        return 0;
    }

    let mut score = parsed.messages.len().saturating_mul(100) + parsed.members.len();
    if hinted {
        score += 500;
    }
    if parsed.platform.eq_ignore_ascii_case(parser_name) {
        score += 250;
    }
    if path_lower.contains(parser_name) {
        score += 25;
    }
    score
}

/// Parser for Xenobot manual-review selection packs.
pub struct ManualReviewParser {
    name_str: String,
}

impl ManualReviewParser {
    /// Creates a new ManualReviewParser instance.
    pub fn new() -> Self {
        Self {
            name_str: "manual-review".to_string(),
        }
    }
}

impl ChatParser for ManualReviewParser {
    fn name(&self) -> &str {
        &self.name_str
    }

    fn can_parse(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy().to_ascii_lowercase();
        path_str.contains("manual-review")
            || path_str.contains("manual_review")
            || path_str.contains("manual-selection")
            || path_str.contains("manual_selection")
    }

    fn parse(&self, path: &Path) -> Result<ParsedChat, ParseError> {
        let content = std::fs::read_to_string(path)?;
        let value: serde_json::Value = serde_json::from_str(&content)?;
        parse_manual_review_value(&value, path).ok_or_else(|| {
            ParseError::UnsupportedFormat("not a xenobot manual-review selection pack".to_string())
        })
    }
}

fn parse_manual_review_value(value: &serde_json::Value, path: &Path) -> Option<ParsedChat> {
    let schema = value_get_string(value, &["schema", "type"])?.to_ascii_lowercase();
    let capture_mode =
        value_get_string(value, &["captureMode", "capture_mode"])?.to_ascii_lowercase();
    if schema != "xenobot/manual-review" || capture_mode != "manual-selection" {
        return None;
    }

    let platform = value_get_string(value, &["platform"])?
        .trim()
        .to_ascii_lowercase();
    if platform.is_empty() {
        return None;
    }

    let chat_name = value_get_string(value, &["chatName", "chat_name"]).unwrap_or_else(|| {
        path.file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "Manual Review".to_string())
    });
    let chat_type = match value_get_string(value, &["chatType", "chat_type"])
        .unwrap_or_else(|| "group".to_string())
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "private" | "direct" | "dm" => ChatType::Private,
        _ => ChatType::Group,
    };

    let mut members = std::collections::BTreeMap::<String, ChatMember>::new();
    if let Some(member_values) = value.get("members").and_then(|entries| entries.as_array()) {
        for raw_member in member_values {
            let id = value_get_string(raw_member, &["id", "senderId", "sender_id"])?;
            upsert_member(
                &mut members,
                id,
                value_get_string(raw_member, &["name", "senderName", "sender_name"]),
                value_get_string(raw_member, &["displayName", "display_name", "nickname"]),
            );
        }
    }

    let mut messages = Vec::new();
    if let Some(message_values) = value
        .get("selectedMessages")
        .and_then(|entries| entries.as_array())
    {
        for raw_message in message_values {
            let sender = value_get_string(raw_message, &["senderId", "sender_id", "sender"])
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "manual-user".to_string());
            let sender_name = value_get_string(
                raw_message,
                &["senderName", "sender_name", "displayName", "display_name"],
            );
            let content =
                value_get_string(raw_message, &["content", "text", "body"]).unwrap_or_default();
            let timestamp = parse_manual_review_timestamp(raw_message).unwrap_or(0);
            let msg_type = parse_manual_review_message_type(raw_message);

            upsert_member(
                &mut members,
                sender.clone(),
                sender_name.clone(),
                sender_name.clone(),
            );
            messages.push(ParsedMessage {
                sender,
                sender_name,
                timestamp,
                content,
                msg_type,
            });
        }
    }

    if let Some(file_values) = value
        .get("selectedFiles")
        .and_then(|entries| entries.as_array())
    {
        for raw_file in file_values {
            let sender = value_get_string(raw_file, &["senderId", "sender_id", "sender"])
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "manual-user".to_string());
            let sender_name = value_get_string(
                raw_file,
                &["senderName", "sender_name", "displayName", "display_name"],
            );
            let timestamp = parse_manual_review_timestamp(raw_file).unwrap_or(0);
            let label = value_get_string(
                raw_file,
                &["label", "name", "title", "path", "filePath", "file_path"],
            )
            .unwrap_or_else(|| "selected-file".to_string());
            let note = value_get_string(raw_file, &["note", "caption", "summary", "reason"]);
            let content = note
                .map(|note| format!("{} | {}", label, note))
                .unwrap_or(label);
            let msg_type = match value_get_string(raw_file, &["kind", "fileKind", "file_kind"])
                .unwrap_or_else(|| "file".to_string())
                .trim()
                .to_ascii_lowercase()
                .as_str()
            {
                "image" | "screenshot" => MessageType::Image,
                "audio" | "voice" => MessageType::Audio,
                "video" => MessageType::Video,
                "link" => MessageType::Link,
                _ => MessageType::File,
            };

            upsert_member(
                &mut members,
                sender.clone(),
                sender_name.clone(),
                sender_name.clone(),
            );
            messages.push(ParsedMessage {
                sender,
                sender_name,
                timestamp,
                content,
                msg_type,
            });
        }
    }

    Some(ParsedChat {
        platform,
        chat_name,
        chat_type,
        messages,
        members: members.into_values().collect(),
    })
}

fn parse_manual_review_timestamp(value: &serde_json::Value) -> Option<i64> {
    use chrono::TimeZone;

    if let Some(ts) = value_get_i64(value, &["timestamp", "ts"]) {
        return Some(ts);
    }

    let raw = value_get_string(value, &["timestamp", "ts"])?;
    chrono::DateTime::parse_from_rfc3339(raw.trim())
        .map(|dt| dt.timestamp())
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(raw.trim(), "%Y-%m-%d %H:%M:%S")
                .map(|dt| chrono::Utc.from_utc_datetime(&dt).timestamp())
        })
        .ok()
}

fn parse_manual_review_message_type(value: &serde_json::Value) -> MessageType {
    match value_get_string(value, &["messageType", "message_type", "type"])
        .unwrap_or_else(|| "text".to_string())
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "image" => MessageType::Image,
        "video" => MessageType::Video,
        "audio" | "voice" => MessageType::Audio,
        "file" | "attachment" => MessageType::File,
        "sticker" | "emoji" => MessageType::Sticker,
        "location" => MessageType::Location,
        "system" => MessageType::System,
        "link" => MessageType::Link,
        _ => MessageType::Text,
    }
}

/// Parser for WhatsApp chat exports.
pub struct WhatsAppParser {
    name_str: String,
}

impl WhatsAppParser {
    /// Creates a new WhatsAppParser instance.
    /// Creates a new registry with default parsers.
    pub fn new() -> Self {
        Self {
            name_str: "whatsapp".to_string(),
        }
    }
}

impl ChatParser for WhatsAppParser {
    fn name(&self) -> &str {
        &self.name_str
    }

    fn can_parse(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy().to_lowercase();
        path_str.contains("whatsapp") || path_str.ends_with(".txt")
    }

    fn parse(&self, path: &Path) -> Result<ParsedChat, ParseError> {
        let content = std::fs::read_to_string(path)?;
        let mut messages = Vec::new();
        let mut members = std::collections::HashSet::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some(msg) = parse_whatsapp_line(line) {
                members.insert(msg.sender.clone());
                messages.push(msg);
            }
        }

        let chat_name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "WhatsApp Chat".to_string());

        Ok(ParsedChat {
            platform: "whatsapp".to_string(),
            chat_name,
            chat_type: ChatType::Group,
            messages,
            members: members
                .into_iter()
                .map(|id| ChatMember {
                    id,
                    name: None,
                    display_name: None,
                })
                .collect(),
        })
    }
}

fn parse_whatsapp_line(line: &str) -> Option<ParsedMessage> {
    let pattern = regex::Regex::new(
        r"^\[?(\d{1,2}/\d{1,2}/\d{2,4}),?\s+(\d{1,2}:\d{2}:\d{2})\]?\s+(.+?):\s+(.*)$",
    )
    .ok()?;

    let caps = pattern.captures(line)?;

    let timestamp_str = format!("{} {}", caps.get(1)?.as_str(), caps.get(2)?.as_str());
    let sender = caps.get(3)?.as_str().to_string();
    let content = caps.get(4)?.as_str().to_string();

    let timestamp = parse_whatsapp_timestamp(&timestamp_str)?;

    Some(ParsedMessage {
        sender,
        sender_name: None,
        timestamp,
        content,
        msg_type: MessageType::Text,
    })
}

fn parse_whatsapp_timestamp(s: &str) -> Option<i64> {
    use chrono::{NaiveDateTime, TimeZone, Utc};
    NaiveDateTime::parse_from_str(s, "%m/%d/%Y %H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(s, "%d/%m/%Y %H:%M:%S"))
        .ok()
        .map(|dt| Utc.from_utc_datetime(&dt).timestamp())
}

/// Parser for LINE chat exports.
pub struct LINEParser {
    name_str: String,
}

impl LINEParser {
    /// Creates a new LINEParser instance.
    /// Creates a new registry with default parsers.
    pub fn new() -> Self {
        Self {
            name_str: "line".to_string(),
        }
    }
}

impl ChatParser for LINEParser {
    fn name(&self) -> &str {
        &self.name_str
    }

    fn can_parse(&self, path: &Path) -> bool {
        path.to_string_lossy().to_lowercase().contains("line")
    }

    fn parse(&self, path: &Path) -> Result<ParsedChat, ParseError> {
        let content = std::fs::read_to_string(path)?;
        let mut messages = Vec::new();
        let mut members = std::collections::HashSet::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some(msg) = parse_line_line(line) {
                members.insert(msg.sender.clone());
                messages.push(msg);
            }
        }

        Ok(ParsedChat {
            platform: "line".to_string(),
            chat_name: path
                .file_stem()
                .ok_or(ParseError::InvalidFormat("missing file stem".to_string()))?
                .to_string_lossy()
                .to_string(),
            chat_type: ChatType::Group,
            messages,
            members: members
                .into_iter()
                .map(|id| ChatMember {
                    id,
                    name: None,
                    display_name: None,
                })
                .collect(),
        })
    }
}

fn parse_line_line(line: &str) -> Option<ParsedMessage> {
    let pattern =
        regex::Regex::new(r"^(\d{4}/\d{2}/\d{2})\s+(\d{2}:\d{2}:\d{2})\s+(.+?)\s+(.*)$").ok()?;
    let caps = pattern.captures(line)?;

    let timestamp_str = format!("{} {}", caps.get(1)?.as_str(), caps.get(2)?.as_str());
    let sender = caps.get(3)?.as_str().to_string();
    let content = caps.get(4)?.as_str().to_string();

    Some(ParsedMessage {
        sender,
        sender_name: None,
        timestamp: parse_line_timestamp(&timestamp_str)?,
        content,
        msg_type: MessageType::Text,
    })
}

fn parse_line_timestamp(s: &str) -> Option<i64> {
    use chrono::{NaiveDateTime, TimeZone, Utc};
    NaiveDateTime::parse_from_str(s, "%Y/%m/%d %H:%M:%S")
        .ok()
        .map(|dt| Utc.from_utc_datetime(&dt).timestamp())
}

fn file_stem_string(path: &Path) -> Result<String, ParseError> {
    Ok(path
        .file_stem()
        .ok_or(ParseError::InvalidFormat("missing file stem".to_string()))?
        .to_string_lossy()
        .to_string())
}

fn value_get_any<'a>(value: &'a serde_json::Value, keys: &[&str]) -> Option<&'a serde_json::Value> {
    let object = value.as_object()?;
    keys.iter().find_map(|key| object.get(*key))
}

fn value_get_string(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    match value_get_any(value, keys)? {
        serde_json::Value::String(v) => Some(v.clone()),
        serde_json::Value::Number(v) => Some(v.to_string()),
        _ => None,
    }
}

fn value_get_i64(value: &serde_json::Value, keys: &[&str]) -> Option<i64> {
    match value_get_any(value, keys)? {
        serde_json::Value::Number(v) => v
            .as_i64()
            .or_else(|| v.as_u64().and_then(|inner| i64::try_from(inner).ok())),
        serde_json::Value::String(v) => v.parse::<i64>().ok(),
        _ => None,
    }
}

fn value_get_bool(value: &serde_json::Value, keys: &[&str]) -> Option<bool> {
    match value_get_any(value, keys)? {
        serde_json::Value::Bool(v) => Some(*v),
        serde_json::Value::Number(v) => Some(v.as_i64()? != 0),
        serde_json::Value::String(v) => match v.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" => Some(true),
            "0" | "false" | "no" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn upsert_member(
    members: &mut std::collections::BTreeMap<String, ChatMember>,
    id: String,
    name: Option<String>,
    display_name: Option<String>,
) {
    let clean_name = name.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });
    let clean_display_name = display_name.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });

    members
        .entry(id.clone())
        .and_modify(|existing| {
            if existing.name.is_none() {
                existing.name = clean_name.clone();
            }
            if existing.display_name.is_none() {
                existing.display_name = clean_display_name.clone();
            }
        })
        .or_insert(ChatMember {
            id,
            name: clean_name,
            display_name: clean_display_name,
        });
}

/// Parser for QQ chat exports.
pub struct QQParser {
    name_str: String,
}

impl QQParser {
    /// Creates a new QQParser instance.
    /// Creates a new registry with default parsers.
    pub fn new() -> Self {
        Self {
            name_str: "qq".to_string(),
        }
    }
}

impl ChatParser for QQParser {
    fn name(&self) -> &str {
        &self.name_str
    }

    fn can_parse(&self, path: &Path) -> bool {
        path.to_string_lossy().to_lowercase().contains("qq")
    }

    fn parse(&self, path: &Path) -> Result<ParsedChat, ParseError> {
        let content = std::fs::read_to_string(path)?;
        let fallback_chat_name = file_stem_string(path)?;

        if let Ok(root) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(parsed) = parse_qq_chat_exporter_json(&root, &fallback_chat_name) {
                return Ok(parsed);
            }
        }

        if let Some(parsed) = parse_qq_official_export(&content, &fallback_chat_name) {
            return Ok(parsed);
        }

        let mut messages = Vec::new();
        let mut members = std::collections::BTreeMap::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some(msg) = parse_qq_line(line) {
                upsert_member(
                    &mut members,
                    msg.sender.clone(),
                    msg.sender_name.clone(),
                    msg.sender_name.clone(),
                );
                messages.push(msg);
            }
        }

        Ok(ParsedChat {
            platform: "qq".to_string(),
            chat_name: fallback_chat_name,
            chat_type: ChatType::Group,
            messages,
            members: members.into_values().collect(),
        })
    }
}

fn parse_qq_line(line: &str) -> Option<ParsedMessage> {
    let pattern =
        regex::Regex::new(r"^\[(\d{4}-\d{2}-\d{2}\s+\d{2}:\d{2}:\d{2})\]\s+(.+?)\s+(.*)$").ok()?;
    let caps = pattern.captures(line)?;

    let timestamp_str = caps.get(1)?.as_str();
    let sender = caps.get(2)?.as_str().to_string();
    let content = caps.get(3)?.as_str().to_string();
    let msg_type = infer_qq_message_type(&content);

    Some(ParsedMessage {
        sender,
        sender_name: None,
        timestamp: parse_qq_timestamp(timestamp_str)?,
        content,
        msg_type,
    })
}

fn parse_qq_timestamp(s: &str) -> Option<i64> {
    use chrono::{NaiveDateTime, TimeZone, Utc};
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|dt| Utc.from_utc_datetime(&dt).timestamp())
}

fn clean_qq_nickname(raw: &str) -> String {
    let prefix = regex::Regex::new(r"^(?:【[^】]*】\s*)+").ok();
    let trimmed = raw.trim();
    match prefix {
        Some(pattern) => {
            let cleaned = pattern.replace(trimmed, "").trim().to_string();
            if cleaned.is_empty() {
                trimmed.to_string()
            } else {
                cleaned
            }
        }
        None => trimmed.to_string(),
    }
}

fn infer_qq_message_type(content: &str) -> MessageType {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return MessageType::System;
    }
    if matches!(trimmed, "[图片]" | "[Image]") {
        return MessageType::Image;
    }
    if matches!(trimmed, "[视频]" | "[Video]") {
        return MessageType::Video;
    }
    if matches!(trimmed, "[语音]" | "[Audio]" | "[Voice]") {
        return MessageType::Audio;
    }
    if matches!(trimmed, "[文件]" | "[File]") {
        return MessageType::File;
    }
    if matches!(trimmed, "[位置]" | "[地理位置]" | "[Location]") {
        return MessageType::Location;
    }
    if matches!(trimmed, "[链接]" | "[卡片消息]" | "[Link]") {
        return MessageType::Link;
    }
    if matches!(trimmed, "[表情]" | "[Sticker]") {
        return MessageType::Sticker;
    }
    if trimmed.contains("加入了群聊")
        || trimmed.contains("退出了群聊")
        || trimmed.contains("被移出群聊")
        || trimmed.contains("修改了群名称")
        || trimmed.contains("撤回了一条消息")
        || trimmed.contains("群公告")
    {
        return MessageType::System;
    }
    MessageType::Text
}

fn parse_qq_official_export(content: &str, fallback_chat_name: &str) -> Option<ParsedChat> {
    let header_pattern = regex::Regex::new(
        r"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2})\s+(.+?)(?:\(([^)]+)\)|<([^>]+)>)?$",
    )
    .ok()?;
    let group_pattern = regex::Regex::new(r"^消息对象:(.+)$").ok()?;

    struct PendingQqMessage {
        sender_id: String,
        sender_name: String,
        timestamp: i64,
        content_lines: Vec<String>,
    }

    let mut chat_name = fallback_chat_name.to_string();
    let mut members = std::collections::BTreeMap::new();
    let mut messages = Vec::new();
    let mut current: Option<PendingQqMessage> = None;
    let mut saw_header = false;

    let push_current =
        |pending: Option<PendingQqMessage>,
         messages: &mut Vec<ParsedMessage>,
         members: &mut std::collections::BTreeMap<String, ChatMember>| {
            if let Some(pending) = pending {
                let content = pending.content_lines.join("\n").trim().to_string();
                if content.is_empty() {
                    return;
                }

                upsert_member(
                    members,
                    pending.sender_id.clone(),
                    Some(pending.sender_name.clone()),
                    Some(pending.sender_name.clone()),
                );
                messages.push(ParsedMessage {
                    sender: pending.sender_id,
                    sender_name: Some(pending.sender_name),
                    timestamp: pending.timestamp,
                    content: content.clone(),
                    msg_type: infer_qq_message_type(&content),
                });
            }
        };

    for raw_line in content.lines() {
        let line = raw_line.trim_end_matches('\r');

        if let Some(captures) = group_pattern.captures(line) {
            if let Some(value) = captures.get(1) {
                let candidate = value.as_str().trim();
                if !candidate.is_empty() {
                    chat_name = candidate.to_string();
                }
            }
            continue;
        }

        if let Some(captures) = header_pattern.captures(line) {
            saw_header = true;
            push_current(current.take(), &mut messages, &mut members);

            let timestamp = parse_qq_timestamp(captures.get(1)?.as_str())?;
            let raw_sender_name = captures.get(2)?.as_str();
            let sender_name = clean_qq_nickname(raw_sender_name);
            let sender_id = captures
                .get(3)
                .or_else(|| captures.get(4))
                .map(|value| value.as_str().trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| sender_name.clone());

            current = Some(PendingQqMessage {
                sender_id,
                sender_name,
                timestamp,
                content_lines: Vec::new(),
            });
            continue;
        }

        if let Some(pending) = current.as_mut() {
            pending.content_lines.push(line.to_string());
        }
    }

    push_current(current, &mut messages, &mut members);

    if !saw_header {
        return None;
    }

    Some(ParsedChat {
        platform: "qq".to_string(),
        chat_name,
        chat_type: ChatType::Group,
        messages,
        members: members.into_values().collect(),
    })
}

fn parse_qq_chat_exporter_json(
    root: &serde_json::Value,
    fallback_chat_name: &str,
) -> Option<ParsedChat> {
    let (chat_name, chat_type, raw_messages) = extract_qce_export_root(root, fallback_chat_name)?;
    let mut parsed_messages = Vec::new();
    let mut members = std::collections::BTreeMap::new();

    for raw_message in raw_messages {
        let Some(parsed) = parse_qce_message_value(raw_message) else {
            continue;
        };
        let (sender_id, account_name, display_name) = extract_qce_sender_identity(raw_message);
        upsert_member(&mut members, sender_id, account_name, display_name);
        parsed_messages.push(parsed);
    }

    if parsed_messages.is_empty() {
        return None;
    }

    Some(ParsedChat {
        platform: "qq".to_string(),
        chat_name,
        chat_type,
        messages: parsed_messages,
        members: members.into_values().collect(),
    })
}

fn extract_qce_export_root<'a>(
    root: &'a serde_json::Value,
    fallback_chat_name: &str,
) -> Option<(String, ChatType, &'a [serde_json::Value])> {
    let messages = root.get("messages")?.as_array()?;
    if !messages.iter().any(looks_like_qce_message) {
        return None;
    }

    let chat_info = root.get("chatInfo");
    let chat_name = chat_info
        .and_then(|value| value_get_string(value, &["name"]))
        .unwrap_or_else(|| fallback_chat_name.to_string());
    let chat_type = chat_info
        .and_then(|value| value_get_string(value, &["type"]))
        .map(|raw| match raw.trim().to_ascii_lowercase().as_str() {
            "friend" | "private" | "direct" | "dm" => ChatType::Private,
            _ => ChatType::Group,
        })
        .unwrap_or(ChatType::Group);

    Some((chat_name, chat_type, messages.as_slice()))
}

fn looks_like_qce_message(value: &serde_json::Value) -> bool {
    value
        .get("sender")
        .and_then(|sender| sender.as_object())
        .is_some()
        && value
            .get("content")
            .and_then(|content| content.as_object())
            .is_some()
        && value_get_any(value, &["timestamp", "time"]).is_some()
}

fn extract_qce_sender_identity(
    value: &serde_json::Value,
) -> (String, Option<String>, Option<String>) {
    let sender = value.get("sender");
    let account_name = sender.and_then(|inner| {
        value_get_string(inner, &["name"])
            .or_else(|| value_get_string(inner, &["nickname"]))
            .or_else(|| value_get_string(inner, &["remark"]))
            .or_else(|| value_get_string(inner, &["groupCard"]))
    });
    let display_name = sender.and_then(|inner| {
        value_get_string(inner, &["groupCard"])
            .or_else(|| value_get_string(inner, &["remark"]))
            .or_else(|| value_get_string(inner, &["name"]))
            .or_else(|| value_get_string(inner, &["nickname"]))
    });
    let sender_id = sender
        .and_then(|inner| {
            value_get_string(inner, &["uid"])
                .or_else(|| value_get_string(inner, &["uin"]))
                .or_else(|| display_name.clone())
                .or_else(|| account_name.clone())
        })
        .unwrap_or_else(|| "qq-unknown".to_string());

    (sender_id, account_name, display_name)
}

fn normalize_epoch_seconds(timestamp: i64) -> i64 {
    if timestamp > 1_000_000_000_000 {
        timestamp / 1000
    } else {
        timestamp
    }
}

fn infer_qce_message_type(value: &serde_json::Value, content_text: &str) -> MessageType {
    if value_get_bool(value, &["system"]).unwrap_or(false) {
        return MessageType::System;
    }

    if let Some(raw_type) = value_get_string(value, &["type"]) {
        match raw_type.trim().to_ascii_lowercase().as_str() {
            "image" => return MessageType::Image,
            "video" => return MessageType::Video,
            "audio" | "voice" => return MessageType::Audio,
            "file" => return MessageType::File,
            "face" | "market_face" | "emoji" | "sticker" => return MessageType::Sticker,
            "location" => return MessageType::Location,
            "json" | "link" | "share" => return MessageType::Link,
            "system" => return MessageType::System,
            _ => {}
        }
    }

    let content = value.get("content");
    if let Some(resources) = content
        .and_then(|inner| inner.get("resources"))
        .and_then(|inner| inner.as_array())
    {
        for resource in resources {
            if let Some(kind) = value_get_string(resource, &["type"]) {
                match kind.trim().to_ascii_lowercase().as_str() {
                    "image" => return MessageType::Image,
                    "video" => return MessageType::Video,
                    "audio" | "voice" => return MessageType::Audio,
                    "file" => return MessageType::File,
                    "emoji" | "face" | "sticker" => return MessageType::Sticker,
                    "location" => return MessageType::Location,
                    "link" | "json" | "card" => return MessageType::Link,
                    _ => {}
                }
            }
        }
    }

    if let Some(elements) = content
        .and_then(|inner| inner.get("elements"))
        .and_then(|inner| inner.as_array())
    {
        for element in elements {
            if let Some(kind) = value_get_string(element, &["type"]) {
                match kind.trim().to_ascii_lowercase().as_str() {
                    "image" => return MessageType::Image,
                    "video" => return MessageType::Video,
                    "audio" | "voice" => return MessageType::Audio,
                    "file" => return MessageType::File,
                    "face" | "market_face" | "emoji" | "sticker" => return MessageType::Sticker,
                    "location" => return MessageType::Location,
                    "json" | "share" | "reply" | "forward" => return MessageType::Link,
                    _ => {}
                }
            }
        }
    }

    infer_qq_message_type(content_text)
}

fn build_qce_message_content(value: &serde_json::Value) -> String {
    let mut segments = Vec::new();

    if let Some(text) = value
        .get("content")
        .and_then(|inner| value_get_string(inner, &["text"]))
        .map(|inner| inner.trim().to_string())
        .filter(|inner| !inner.is_empty())
    {
        segments.push(text);
    }

    if let Some(resources) = value
        .get("content")
        .and_then(|inner| inner.get("resources"))
        .and_then(|inner| inner.as_array())
    {
        for resource in resources {
            let kind = value_get_string(resource, &["type"])
                .unwrap_or_else(|| "file".to_string())
                .trim()
                .to_ascii_lowercase();
            let file_name = value_get_string(resource, &["filename", "name", "localPath"])
                .unwrap_or_else(|| "resource".to_string());
            let label = match kind.as_str() {
                "image" => format!("[Image: {}]", file_name),
                "video" => format!("[Video: {}]", file_name),
                "audio" | "voice" => format!("[Audio: {}]", file_name),
                "emoji" | "face" | "sticker" => format!("[Sticker: {}]", file_name),
                "location" => format!("[Location: {}]", file_name),
                "json" | "card" | "link" => format!("[Link: {}]", file_name),
                _ => format!("[File: {}]", file_name),
            };
            segments.push(label);
        }
    }

    if segments.is_empty() && value_get_bool(value, &["recalled"]).unwrap_or(false) {
        segments.push("[Recalled message]".to_string());
    }

    if segments.is_empty() && value_get_bool(value, &["system"]).unwrap_or(false) {
        segments.push("[System message]".to_string());
    }

    segments.join("\n").trim().to_string()
}

fn parse_qce_message_value(value: &serde_json::Value) -> Option<ParsedMessage> {
    if !looks_like_qce_message(value) {
        return None;
    }

    let (sender, account_name, display_name) = extract_qce_sender_identity(value);
    let sender_name = display_name.or(account_name);
    let content = build_qce_message_content(value);
    if content.is_empty() {
        return None;
    }

    let timestamp = value_get_i64(value, &["timestamp"])
        .map(normalize_epoch_seconds)
        .or_else(|| {
            value_get_string(value, &["time"]).and_then(|raw| {
                chrono::DateTime::parse_from_rfc3339(raw.trim())
                    .map(|inner| inner.timestamp())
                    .ok()
            })
        })
        .unwrap_or(0);
    let msg_type = infer_qce_message_type(value, &content);

    Some(ParsedMessage {
        sender,
        sender_name,
        timestamp,
        content,
        msg_type,
    })
}

/// Parser for Telegram chat exports.
pub struct TelegramParser {
    name_str: String,
}

impl TelegramParser {
    /// Creates a new TelegramParser instance.
    /// Creates a new registry with default parsers.
    pub fn new() -> Self {
        Self {
            name_str: "telegram".to_string(),
        }
    }
}

impl ChatParser for TelegramParser {
    fn name(&self) -> &str {
        &self.name_str
    }

    fn can_parse(&self, path: &Path) -> bool {
        path.to_string_lossy().to_lowercase().contains("telegram")
            || path.extension().map(|e| e == "json").unwrap_or(false)
    }

    fn parse(&self, path: &Path) -> Result<ParsedChat, ParseError> {
        let content = std::fs::read_to_string(path)?;

        #[derive(Deserialize)]
        struct TelegramExport {
            name: Option<String>,
            #[serde(default)]
            messages: Vec<TelegramMessage>,
        }

        #[derive(Deserialize)]
        struct TelegramMessage {
            #[serde(rename = "from")]
            from: Option<String>,
            #[serde(rename = "from_id")]
            from_id: Option<String>,
            date: String,
            text: Option<String>,
        }

        let export: TelegramExport = serde_json::from_str(&content)?;

        let messages: Vec<ParsedMessage> = export
            .messages
            .iter()
            .filter_map(|msg| {
                let sender = msg
                    .from
                    .clone()
                    .or(msg.from_id.clone())
                    .unwrap_or_else(|| "Unknown".to_string());
                let content = msg.text.clone().unwrap_or_default();
                if content.is_empty() {
                    return None;
                }

                Some(ParsedMessage {
                    sender,
                    sender_name: None,
                    timestamp: parse_telegram_timestamp(&msg.date).unwrap_or(0),
                    content,
                    msg_type: MessageType::Text,
                })
            })
            .collect();

        Ok(ParsedChat {
            platform: "telegram".to_string(),
            chat_name: export.name.unwrap_or_else(|| "Telegram Chat".to_string()),
            chat_type: ChatType::Group,
            messages,
            members: vec![],
        })
    }
}

fn parse_telegram_timestamp(s: &str) -> Option<i64> {
    use chrono::DateTime;
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.timestamp())
}

/// Parser for Discord chat exports.
pub struct DiscordParser {
    name_str: String,
}

impl DiscordParser {
    /// Creates a new DiscordParser instance.
    /// Creates a new registry with default parsers.
    pub fn new() -> Self {
        Self {
            name_str: "discord".to_string(),
        }
    }
}

impl ChatParser for DiscordParser {
    fn name(&self) -> &str {
        &self.name_str
    }

    fn can_parse(&self, path: &Path) -> bool {
        path.to_string_lossy().to_lowercase().contains("discord")
    }

    fn parse(&self, path: &Path) -> Result<ParsedChat, ParseError> {
        let content = std::fs::read_to_string(path)?;
        let root: serde_json::Value = serde_json::from_str(&content)?;
        let default_chat_name = file_stem_string(path)?;
        let (chat_name, chat_type, raw_messages) =
            extract_discord_export_root(&root, &default_chat_name)?;
        let mut parsed_messages = Vec::new();
        let mut members = std::collections::BTreeMap::new();

        for raw_message in raw_messages {
            if !looks_like_discord_message(raw_message) {
                continue;
            }

            let Some(author) = value_get_any(raw_message, &["Author", "author"]) else {
                continue;
            };
            let (account_name, display_name) = extract_discord_author_names(author);
            let sender_name = display_name.clone().or(account_name.clone());
            let sender = value_get_string(author, &["ID", "id"])
                .or_else(|| sender_name.clone())
                .unwrap_or_else(|| "discord-unknown".to_string());
            let timestamp = value_get_string(raw_message, &["Timestamp", "timestamp"])
                .and_then(|value| parse_discord_timestamp(&value))
                .unwrap_or(0);

            let mut parts = Vec::new();
            if let Some(content) = value_get_string(raw_message, &["Content", "content"]) {
                if !content.trim().is_empty() {
                    parts.push(content.trim().to_string());
                }
            }

            let attachment_type = append_discord_attachments(raw_message, &mut parts);
            let has_embed = append_discord_embeds(raw_message, &mut parts);
            let has_sticker = append_discord_stickers(raw_message, &mut parts);
            let raw_type = value_get_string(raw_message, &["Type", "type"]);
            let msg_type = infer_discord_message_type(
                raw_type.as_deref(),
                attachment_type,
                has_sticker,
                has_embed,
            );

            if parts.is_empty() {
                if let Some(type_name) = raw_type {
                    if matches!(msg_type, MessageType::System) {
                        parts.push(format!("[System: {}]", type_name));
                    }
                }
            }

            let content = parts.join("\n");
            if content.trim().is_empty() {
                continue;
            }

            upsert_member(&mut members, sender.clone(), account_name, display_name);
            parsed_messages.push(ParsedMessage {
                sender,
                sender_name,
                timestamp,
                content,
                msg_type,
            });
        }

        Ok(ParsedChat {
            platform: "discord".to_string(),
            chat_name,
            chat_type,
            messages: parsed_messages,
            members: members.into_values().collect(),
        })
    }
}

fn parse_discord_timestamp(s: &str) -> Option<i64> {
    use chrono::DateTime;
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.timestamp())
}

fn extract_discord_export_root<'a>(
    root: &'a serde_json::Value,
    fallback_chat_name: &str,
) -> Result<(String, ChatType, &'a [serde_json::Value]), ParseError> {
    match root {
        serde_json::Value::Array(items) => {
            if items.iter().any(looks_like_discord_message) {
                Ok((
                    fallback_chat_name.to_string(),
                    ChatType::Group,
                    items.as_slice(),
                ))
            } else {
                Err(ParseError::InvalidFormat(
                    "discord export array did not contain discord-like messages".to_string(),
                ))
            }
        }
        serde_json::Value::Object(_) => {
            let Some(messages) =
                value_get_any(root, &["messages"]).and_then(|value| value.as_array())
            else {
                return Err(ParseError::InvalidFormat(
                    "discord export root missing messages array".to_string(),
                ));
            };

            if !messages.iter().any(looks_like_discord_message) {
                return Err(ParseError::InvalidFormat(
                    "discord export messages array did not match exporter shape".to_string(),
                ));
            }

            let guild_name = value_get_any(root, &["guild"])
                .and_then(|value| value_get_string(value, &["name", "Name"]));
            let channel_name = value_get_any(root, &["channel"])
                .and_then(|value| value_get_string(value, &["name", "Name"]))
                .unwrap_or_else(|| fallback_chat_name.to_string());
            let channel_type = value_get_any(root, &["channel"])
                .and_then(|value| value_get_string(value, &["type", "Type"]))
                .unwrap_or_default();
            let chat_type = if channel_type.eq_ignore_ascii_case("directmessage")
                || channel_type.eq_ignore_ascii_case("dm")
            {
                ChatType::Private
            } else {
                ChatType::Group
            };
            let chat_name = guild_name
                .map(|guild| format!("{} / #{}", guild, channel_name))
                .unwrap_or(channel_name);
            Ok((chat_name, chat_type, messages.as_slice()))
        }
        _ => Err(ParseError::InvalidFormat(
            "discord export must be a JSON array or object root".to_string(),
        )),
    }
}

fn looks_like_discord_message(value: &serde_json::Value) -> bool {
    value_get_any(value, &["Author", "author"]).is_some()
        && value_get_any(value, &["Timestamp", "timestamp"]).is_some()
}

fn extract_discord_author_names(author: &serde_json::Value) -> (Option<String>, Option<String>) {
    let account_name = value_get_string(
        author,
        &[
            "Name", "name", "username", "Username", "userName", "UserName",
        ],
    );
    let display_name = value_get_string(
        author,
        &[
            "nickname",
            "Nickname",
            "nickName",
            "NickName",
            "displayName",
            "DisplayName",
            "display_name",
            "globalName",
            "GlobalName",
            "global_name",
        ],
    )
    .or_else(|| account_name.clone());

    (account_name, display_name)
}

fn discord_attachment_marker(file_name: &str) -> (String, MessageType) {
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
        || lower.ends_with(".webp")
        || lower.ends_with(".bmp")
        || lower.ends_with(".svg")
    {
        return (format!("[Image: {}]", file_name), MessageType::Image);
    }
    if lower.ends_with(".mp4")
        || lower.ends_with(".webm")
        || lower.ends_with(".mov")
        || lower.ends_with(".avi")
        || lower.ends_with(".mkv")
    {
        return (format!("[Video: {}]", file_name), MessageType::Video);
    }
    if lower.ends_with(".mp3")
        || lower.ends_with(".wav")
        || lower.ends_with(".ogg")
        || lower.ends_with(".flac")
        || lower.ends_with(".m4a")
    {
        return (format!("[Audio: {}]", file_name), MessageType::Audio);
    }
    (format!("[File: {}]", file_name), MessageType::File)
}

fn append_discord_attachments(
    message: &serde_json::Value,
    parts: &mut Vec<String>,
) -> Option<MessageType> {
    let attachments = value_get_any(message, &["Attachments", "attachments"])?.as_array()?;
    let mut dominant = None;
    for attachment in attachments {
        let file_name = value_get_string(attachment, &["fileName", "filename", "FileName", "name"])
            .unwrap_or_else(|| "attachment".to_string());
        let (marker, kind) = discord_attachment_marker(&file_name);
        parts.push(marker);
        if dominant.is_none() {
            dominant = Some(kind);
        }
    }
    dominant
}

fn append_discord_embeds(message: &serde_json::Value, parts: &mut Vec<String>) -> bool {
    let Some(embeds) =
        value_get_any(message, &["Embeds", "embeds"]).and_then(|value| value.as_array())
    else {
        return false;
    };

    let mut appended = false;
    for embed in embeds {
        let title = value_get_string(embed, &["title", "Title"])
            .or_else(|| value_get_string(embed, &["url", "Url"]))
            .or_else(|| value_get_string(embed, &["description", "Description"]));
        if let Some(title) = title {
            parts.push(format!("[Link: {}]", title.trim()));
            appended = true;
        }
    }
    appended
}

fn append_discord_stickers(message: &serde_json::Value, parts: &mut Vec<String>) -> bool {
    let Some(stickers) =
        value_get_any(message, &["Stickers", "stickers"]).and_then(|value| value.as_array())
    else {
        return false;
    };

    let mut appended = false;
    for sticker in stickers {
        if let Some(name) = value_get_string(sticker, &["name", "Name"]) {
            parts.push(format!("[Sticker: {}]", name.trim()));
            appended = true;
        }
    }
    appended
}

fn infer_discord_message_type(
    raw_type: Option<&str>,
    attachment_type: Option<MessageType>,
    has_sticker: bool,
    has_embed: bool,
) -> MessageType {
    if let Some(raw_type) = raw_type {
        match raw_type {
            "Default"
            | "default"
            | "Reply"
            | "reply"
            | "ThreadStarterMessage"
            | "threadStarterMessage" => {}
            "ChannelPinnedMessage"
            | "channelPinnedMessage"
            | "UserJoin"
            | "userJoin"
            | "RecipientAdd"
            | "recipientAdd"
            | "RecipientRemove"
            | "recipientRemove"
            | "GuildBoost"
            | "guildBoost"
            | "Call"
            | "call"
            | "AutoModerationAction"
            | "autoModerationAction"
            | "ThreadCreated"
            | "threadCreated"
            | "ChatInputCommand"
            | "chatInputCommand" => return MessageType::System,
            _ => {}
        }
    }

    if let Some(kind) = attachment_type {
        return kind;
    }
    if has_sticker {
        return MessageType::Sticker;
    }
    if has_embed {
        return MessageType::Link;
    }
    MessageType::Text
}

/// Parser for WeChat/WeFlow chat exports.
pub struct WeChatParser {
    name_str: String,
}

impl WeChatParser {
    /// Creates a new WeChatParser instance.
    /// Creates a new registry with default parsers.
    pub fn new() -> Self {
        Self {
            name_str: "wechat".to_string(),
        }
    }
}

impl ChatParser for WeChatParser {
    fn name(&self) -> &str {
        &self.name_str
    }

    fn can_parse(&self, path: &Path) -> bool {
        path.to_string_lossy().to_lowercase().contains("wechat")
            || path.to_string_lossy().to_lowercase().contains("weflow")
    }

    fn parse(&self, path: &Path) -> Result<ParsedChat, ParseError> {
        let content = std::fs::read_to_string(path)?;
        let root: serde_json::Value = serde_json::from_str(&content)?;
        let default_chat_name = file_stem_string(path)?;
        let (chat_name, chat_type, raw_messages) =
            extract_wechat_export_root(&root, &default_chat_name)?;
        let mut parsed_messages = Vec::new();
        let mut members = std::collections::BTreeMap::new();

        for raw_message in raw_messages {
            let Some(parsed) = parse_wechat_message_value(raw_message) else {
                continue;
            };
            upsert_member(
                &mut members,
                parsed.sender.clone(),
                parsed.sender_name.clone(),
                parsed.sender_name.clone(),
            );
            parsed_messages.push(parsed);
        }

        Ok(ParsedChat {
            platform: "wechat".to_string(),
            chat_name,
            chat_type,
            messages: parsed_messages,
            members: members.into_values().collect(),
        })
    }
}

fn extract_wechat_export_root<'a>(
    root: &'a serde_json::Value,
    fallback_chat_name: &str,
) -> Result<(String, ChatType, &'a [serde_json::Value]), ParseError> {
    match root {
        serde_json::Value::Array(items) => {
            if items.iter().any(looks_like_wechat_message) {
                Ok((
                    fallback_chat_name.to_string(),
                    ChatType::Group,
                    items.as_slice(),
                ))
            } else {
                Err(ParseError::InvalidFormat(
                    "wechat export array did not contain wechat-like messages".to_string(),
                ))
            }
        }
        serde_json::Value::Object(_) => {
            let Some(messages) =
                value_get_any(root, &["messages"]).and_then(|value| value.as_array())
            else {
                return Err(ParseError::InvalidFormat(
                    "wechat export root missing messages array".to_string(),
                ));
            };

            if !messages.iter().any(looks_like_wechat_message) {
                return Err(ParseError::InvalidFormat(
                    "wechat export messages array did not match WeFlow shape".to_string(),
                ));
            }

            let session = value_get_any(root, &["session"]);
            let chat_name = session
                .and_then(|value| {
                    value_get_string(value, &["displayName", "display_name"])
                        .or_else(|| value_get_string(value, &["remark"]))
                        .or_else(|| value_get_string(value, &["nickname"]))
                        .or_else(|| value_get_string(value, &["wxid"]))
                })
                .or_else(|| value_get_string(root, &["talker"]))
                .unwrap_or_else(|| fallback_chat_name.to_string());
            let chat_type = session
                .and_then(|value| value_get_string(value, &["type"]))
                .map(|raw| {
                    if raw.contains("私聊") || raw.eq_ignore_ascii_case("private") {
                        ChatType::Private
                    } else {
                        ChatType::Group
                    }
                })
                .or_else(|| {
                    value_get_string(root, &["talker"]).map(|talker| {
                        if talker.ends_with("@chatroom") {
                            ChatType::Group
                        } else {
                            ChatType::Private
                        }
                    })
                })
                .unwrap_or(ChatType::Group);

            Ok((chat_name, chat_type, messages.as_slice()))
        }
        _ => Err(ParseError::InvalidFormat(
            "wechat export must be a JSON array or object root".to_string(),
        )),
    }
}

fn looks_like_wechat_message(value: &serde_json::Value) -> bool {
    value_get_any(value, &["create_time", "createTime"]).is_some()
        && value_get_any(
            value,
            &[
                "sender_id",
                "sender_name",
                "senderUsername",
                "senderDisplayName",
                "is_sender",
                "isSend",
            ],
        )
        .is_some()
}

fn infer_wechat_message_type(value: &serde_json::Value) -> MessageType {
    if let Some(raw_type) =
        value_get_string(value, &["typeName", "msgTypeName", "type_name", "type"])
    {
        match raw_type.as_str() {
            "图片消息" | "image" | "Image" => return MessageType::Image,
            "语音消息" | "audio" | "Audio" | "voice" | "Voice" => return MessageType::Audio,
            "视频消息" | "video" | "Video" => return MessageType::Video,
            "文件消息" | "file" | "File" => return MessageType::File,
            "位置消息" | "location" | "Location" => return MessageType::Location,
            "系统消息" | "system" | "System" => return MessageType::System,
            "卡片式链接" | "图文消息" | "link" | "Link" => return MessageType::Link,
            "动画表情" | "sticker" | "Sticker" => return MessageType::Sticker,
            _ => {}
        }
    }

    match value_get_i64(value, &["type"]) {
        Some(3) => MessageType::Image,
        Some(34) => MessageType::Audio,
        Some(43) => MessageType::Video,
        Some(47) => MessageType::Sticker,
        Some(48) => MessageType::Location,
        Some(49) => MessageType::Link,
        Some(10000) => MessageType::System,
        _ => match value_get_i64(value, &["localType"]) {
            Some(3) => MessageType::Image,
            Some(34) => MessageType::Audio,
            Some(43) => MessageType::Video,
            Some(47) => MessageType::Sticker,
            Some(48) => MessageType::Location,
            Some(49) => MessageType::Link,
            Some(10000) => MessageType::System,
            _ => MessageType::Text,
        },
    }
}

fn parse_wechat_message_value(value: &serde_json::Value) -> Option<ParsedMessage> {
    if !looks_like_wechat_message(value) {
        return None;
    }

    let content = value_get_string(value, &["parsedContent"])
        .or_else(|| value_get_string(value, &["content"]))
        .or_else(|| value_get_string(value, &["rawContent"]))?
        .trim()
        .to_string();
    if content.is_empty() {
        return None;
    }

    let sender_name = value_get_string(value, &["sender_name", "senderDisplayName"]);
    let sender_id = value_get_string(value, &["sender_id", "senderUsername"]);
    let sent_by_self = value_get_bool(value, &["is_sender", "isSend"]).unwrap_or(false);
    let sender = if sent_by_self {
        sender_id.clone().unwrap_or_else(|| "self".to_string())
    } else {
        sender_id
            .clone()
            .or_else(|| sender_name.clone())
            .unwrap_or_else(|| "wechat-unknown".to_string())
    };

    Some(ParsedMessage {
        sender,
        sender_name,
        timestamp: value_get_i64(value, &["create_time", "createTime"]).unwrap_or(0),
        content,
        msg_type: infer_wechat_message_type(value),
    })
}

/// Parser for Instagram chat exports.
pub struct InstagramParser {
    name_str: String,
}

impl InstagramParser {
    /// Creates a new InstagramParser instance.
    /// Creates a new registry with default parsers.
    pub fn new() -> Self {
        Self {
            name_str: "instagram".to_string(),
        }
    }
}

impl ChatParser for InstagramParser {
    fn name(&self) -> &str {
        &self.name_str
    }

    fn can_parse(&self, path: &Path) -> bool {
        path.to_string_lossy().to_lowercase().contains("instagram")
    }

    fn parse(&self, path: &Path) -> Result<ParsedChat, ParseError> {
        let content = std::fs::read_to_string(path)?;

        #[derive(Deserialize)]
        struct InstagramMessage {
            #[serde(rename = "sender")]
            sender: Option<String>,
            #[serde(rename = "timestamp")]
            timestamp: i64,
            #[serde(rename = "content")]
            content: Option<String>,
        }

        let messages: Vec<InstagramMessage> = serde_json::from_str(&content)?;

        let parsed_messages: Vec<ParsedMessage> = messages
            .iter()
            .filter_map(|msg| {
                let sender = msg.sender.clone()?;
                let content = msg.content.clone()?;
                if content.is_empty() {
                    return None;
                }

                Some(ParsedMessage {
                    sender,
                    sender_name: None,
                    timestamp: msg.timestamp,
                    content,
                    msg_type: MessageType::Text,
                })
            })
            .collect();

        Ok(ParsedChat {
            platform: "instagram".to_string(),
            chat_name: path
                .file_stem()
                .ok_or(ParseError::InvalidFormat("missing file stem".to_string()))?
                .to_string_lossy()
                .to_string(),
            chat_type: ChatType::Private,
            messages: parsed_messages,
            members: vec![],
        })
    }
}

/// Parser for iMessage chat exports.
///
/// Parses iMessage chat database exports from macOS.
pub struct IMessageParser {
    name_str: String,
}

impl IMessageParser {
    /// Creates a new IMessageParser instance.
    pub fn new() -> Self {
        Self {
            name_str: "imessage".to_string(),
        }
    }
}

impl ChatParser for IMessageParser {
    fn name(&self) -> &str {
        &self.name_str
    }

    fn can_parse(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy().to_lowercase();
        path_str.contains("imessage") || path_str.contains("messages")
    }

    fn parse(&self, path: &Path) -> Result<ParsedChat, ParseError> {
        let content = std::fs::read_to_string(path)?;

        #[derive(Deserialize)]
        struct IMessage {
            text: Option<String>,
            sender: Option<String>,
            date: Option<String>,
        }

        let messages: Vec<IMessage> = serde_json::from_str(&content).unwrap_or_else(|_| vec![]);

        let parsed_messages: Vec<ParsedMessage> = messages
            .iter()
            .filter_map(|msg| {
                let sender = msg.sender.clone()?;
                let content = msg.text.clone()?;
                if content.is_empty() {
                    return None;
                }

                Some(ParsedMessage {
                    sender,
                    sender_name: None,
                    timestamp: parse_imessage_timestamp(msg.date.as_deref()).unwrap_or(0),
                    content,
                    msg_type: MessageType::Text,
                })
            })
            .collect();

        Ok(ParsedChat {
            platform: "imessage".to_string(),
            chat_name: path
                .file_stem()
                .ok_or(ParseError::InvalidFormat("missing file stem".to_string()))?
                .to_string_lossy()
                .to_string(),
            chat_type: ChatType::Private,
            messages: parsed_messages,
            members: vec![],
        })
    }
}

fn parse_imessage_timestamp(s: Option<&str>) -> Option<i64> {
    use chrono::DateTime;
    s.and_then(|s| DateTime::parse_from_rfc3339(s).ok())?
        .timestamp()
        .into()
}

/// Parser for Facebook Messenger chat exports.
///
/// Parses Messenger chat export files.
pub struct MessengerParser {
    name_str: String,
}

impl MessengerParser {
    /// Creates a new MessengerParser instance.
    pub fn new() -> Self {
        Self {
            name_str: "messenger".to_string(),
        }
    }
}

impl ChatParser for MessengerParser {
    fn name(&self) -> &str {
        &self.name_str
    }

    fn can_parse(&self, path: &Path) -> bool {
        path.to_string_lossy().to_lowercase().contains("messenger")
            || path.to_string_lossy().to_lowercase().contains("facebook")
    }

    fn parse(&self, path: &Path) -> Result<ParsedChat, ParseError> {
        let content = std::fs::read_to_string(path)?;

        #[derive(Deserialize)]
        struct MessengerMessage {
            sender_name: Option<String>,
            timestamp_ms: Option<i64>,
            content: Option<String>,
        }

        let messages: Vec<MessengerMessage> =
            serde_json::from_str(&content).unwrap_or_else(|_| vec![]);

        let parsed_messages: Vec<ParsedMessage> = messages
            .iter()
            .filter_map(|msg| {
                let sender = msg.sender_name.clone()?;
                let content = msg.content.clone()?;
                if content.is_empty() {
                    return None;
                }

                Some(ParsedMessage {
                    sender,
                    sender_name: None,
                    timestamp: msg.timestamp_ms.unwrap_or(0) / 1000,
                    content,
                    msg_type: MessageType::Text,
                })
            })
            .collect();

        Ok(ParsedChat {
            platform: "messenger".to_string(),
            chat_name: path
                .file_stem()
                .ok_or(ParseError::InvalidFormat("missing file stem".to_string()))?
                .to_string_lossy()
                .to_string(),
            chat_type: ChatType::Private,
            messages: parsed_messages,
            members: vec![],
        })
    }
}

/// Parser for KakaoTalk chat exports.
///
/// Parses KakaoTalk chat export files.
pub struct KakaoTalkParser {
    name_str: String,
}

impl KakaoTalkParser {
    /// Creates a new KakaoTalkParser instance.
    pub fn new() -> Self {
        Self {
            name_str: "kakaotalk".to_string(),
        }
    }
}

impl ChatParser for KakaoTalkParser {
    fn name(&self) -> &str {
        &self.name_str
    }

    fn can_parse(&self, path: &Path) -> bool {
        path.to_string_lossy().to_lowercase().contains("kakao")
            || path.to_string_lossy().to_lowercase().contains("kakaotalk")
    }

    fn parse(&self, path: &Path) -> Result<ParsedChat, ParseError> {
        let content = std::fs::read_to_string(path)?;

        #[derive(Deserialize)]
        struct KakaoMessage {
            sender: Option<String>,
            message: Option<String>,
            date: Option<String>,
        }

        let messages: Vec<KakaoMessage> = serde_json::from_str(&content).unwrap_or_else(|_| vec![]);

        let parsed_messages: Vec<ParsedMessage> = messages
            .iter()
            .filter_map(|msg| {
                let sender = msg.sender.clone()?;
                let content = msg.message.clone()?;
                if content.is_empty() {
                    return None;
                }

                Some(ParsedMessage {
                    sender,
                    sender_name: None,
                    timestamp: parse_kakao_timestamp(msg.date.as_deref()).unwrap_or(0),
                    content,
                    msg_type: MessageType::Text,
                })
            })
            .collect();

        Ok(ParsedChat {
            platform: "kakaotalk".to_string(),
            chat_name: path
                .file_stem()
                .ok_or(ParseError::InvalidFormat("missing file stem".to_string()))?
                .to_string_lossy()
                .to_string(),
            chat_type: ChatType::Group,
            messages: parsed_messages,
            members: vec![],
        })
    }
}

fn parse_kakao_timestamp(s: Option<&str>) -> Option<i64> {
    use chrono::{NaiveDateTime, TimeZone, Utc};
    s.and_then(|s| NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").ok())
        .map(|dt| Utc.from_utc_datetime(&dt).timestamp())
}

/// Parser for Slack chat exports.
///
/// Parses Slack chat export files.
pub struct SlackParser {
    name_str: String,
}

impl SlackParser {
    /// Creates a new SlackParser instance.
    pub fn new() -> Self {
        Self {
            name_str: "slack".to_string(),
        }
    }
}

impl ChatParser for SlackParser {
    fn name(&self) -> &str {
        &self.name_str
    }

    fn can_parse(&self, path: &Path) -> bool {
        path.to_string_lossy().to_lowercase().contains("slack")
    }

    fn parse(&self, path: &Path) -> Result<ParsedChat, ParseError> {
        let content = std::fs::read_to_string(path)?;

        #[derive(Deserialize)]
        struct SlackMessage {
            user: Option<String>,
            ts: Option<String>,
            text: Option<String>,
        }

        let messages: Vec<SlackMessage> = serde_json::from_str(&content).unwrap_or_else(|_| vec![]);

        let parsed_messages: Vec<ParsedMessage> = messages
            .iter()
            .filter_map(|msg| {
                let sender = msg.user.clone()?;
                let content = msg.text.clone()?;
                if content.is_empty() {
                    return None;
                }

                Some(ParsedMessage {
                    sender,
                    sender_name: None,
                    timestamp: parse_slack_timestamp(msg.ts.as_deref()).unwrap_or(0),
                    content,
                    msg_type: MessageType::Text,
                })
            })
            .collect();

        Ok(ParsedChat {
            platform: "slack".to_string(),
            chat_name: path
                .file_stem()
                .ok_or(ParseError::InvalidFormat("missing file stem".to_string()))?
                .to_string_lossy()
                .to_string(),
            chat_type: ChatType::Group,
            messages: parsed_messages,
            members: vec![],
        })
    }
}

fn parse_slack_timestamp(s: Option<&str>) -> Option<i64> {
    s.and_then(|s| s.parse::<f64>().ok().map(|v| v.floor() as i64))
}

/// Parser for Microsoft Teams chat exports.
///
/// Parses Teams chat export files.
pub struct TeamsParser {
    name_str: String,
}

impl TeamsParser {
    /// Creates a new TeamsParser instance.
    pub fn new() -> Self {
        Self {
            name_str: "teams".to_string(),
        }
    }
}

impl ChatParser for TeamsParser {
    fn name(&self) -> &str {
        &self.name_str
    }

    fn can_parse(&self, path: &Path) -> bool {
        path.to_string_lossy().to_lowercase().contains("teams")
            || path.to_string_lossy().to_lowercase().contains("microsoft")
    }

    fn parse(&self, path: &Path) -> Result<ParsedChat, ParseError> {
        let content = std::fs::read_to_string(path)?;

        #[derive(Deserialize)]
        struct TeamsMessage {
            from: Option<String>,
            date: Option<String>,
            content: Option<String>,
        }

        let messages: Vec<TeamsMessage> = serde_json::from_str(&content).unwrap_or_else(|_| vec![]);

        let parsed_messages: Vec<ParsedMessage> = messages
            .iter()
            .filter_map(|msg| {
                let sender = msg.from.clone()?;
                let content = msg.content.clone()?;
                if content.is_empty() {
                    return None;
                }

                Some(ParsedMessage {
                    sender,
                    sender_name: None,
                    timestamp: parse_teams_timestamp(msg.date.as_deref()).unwrap_or(0),
                    content,
                    msg_type: MessageType::Text,
                })
            })
            .collect();

        Ok(ParsedChat {
            platform: "teams".to_string(),
            chat_name: path
                .file_stem()
                .ok_or(ParseError::InvalidFormat("missing file stem".to_string()))?
                .to_string_lossy()
                .to_string(),
            chat_type: ChatType::Group,
            messages: parsed_messages,
            members: vec![],
        })
    }
}

fn parse_teams_timestamp(s: Option<&str>) -> Option<i64> {
    use chrono::DateTime;
    s.and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.timestamp())
}

/// Parser for Signal chat exports.
///
/// Parses Signal chat export files.
pub struct SignalParser {
    name_str: String,
}

impl SignalParser {
    /// Creates a new SignalParser instance.
    pub fn new() -> Self {
        Self {
            name_str: "signal".to_string(),
        }
    }
}

impl ChatParser for SignalParser {
    fn name(&self) -> &str {
        &self.name_str
    }

    fn can_parse(&self, path: &Path) -> bool {
        path.to_string_lossy().to_lowercase().contains("signal")
    }

    fn parse(&self, path: &Path) -> Result<ParsedChat, ParseError> {
        let content = std::fs::read_to_string(path)?;

        #[derive(Deserialize)]
        struct SignalMessage {
            sender: Option<String>,
            timestamp: Option<i64>,
            body: Option<String>,
        }

        let messages: Vec<SignalMessage> =
            serde_json::from_str(&content).unwrap_or_else(|_| vec![]);

        let parsed_messages: Vec<ParsedMessage> = messages
            .iter()
            .filter_map(|msg| {
                let sender = msg.sender.clone()?;
                let content = msg.body.clone()?;
                if content.is_empty() {
                    return None;
                }

                Some(ParsedMessage {
                    sender,
                    sender_name: None,
                    timestamp: msg.timestamp.unwrap_or(0) / 1000,
                    content,
                    msg_type: MessageType::Text,
                })
            })
            .collect();

        Ok(ParsedChat {
            platform: "signal".to_string(),
            chat_name: path
                .file_stem()
                .ok_or(ParseError::InvalidFormat("missing file stem".to_string()))?
                .to_string_lossy()
                .to_string(),
            chat_type: ChatType::Private,
            messages: parsed_messages,
            members: vec![],
        })
    }
}

/// Parser for Skype chat exports.
///
/// Parses Skype chat export files.
pub struct SkypeParser {
    name_str: String,
}

impl SkypeParser {
    /// Creates a new SkypeParser instance.
    pub fn new() -> Self {
        Self {
            name_str: "skype".to_string(),
        }
    }
}

impl ChatParser for SkypeParser {
    fn name(&self) -> &str {
        &self.name_str
    }

    fn can_parse(&self, path: &Path) -> bool {
        path.to_string_lossy().to_lowercase().contains("skype")
    }

    fn parse(&self, path: &Path) -> Result<ParsedChat, ParseError> {
        let content = std::fs::read_to_string(path)?;

        #[derive(Deserialize)]
        struct SkypeMessage {
            sender: Option<String>,
            datetime: Option<String>,
            msg_content: Option<String>,
        }

        let messages: Vec<SkypeMessage> = serde_json::from_str(&content).unwrap_or_else(|_| vec![]);

        let parsed_messages: Vec<ParsedMessage> = messages
            .iter()
            .filter_map(|msg| {
                let sender = msg.sender.clone()?;
                let content = msg.msg_content.clone()?;
                if content.is_empty() {
                    return None;
                }

                Some(ParsedMessage {
                    sender,
                    sender_name: None,
                    timestamp: parse_skype_timestamp(msg.datetime.as_deref()).unwrap_or(0),
                    content,
                    msg_type: MessageType::Text,
                })
            })
            .collect();

        Ok(ParsedChat {
            platform: "skype".to_string(),
            chat_name: path
                .file_stem()
                .ok_or(ParseError::InvalidFormat("missing file stem".to_string()))?
                .to_string_lossy()
                .to_string(),
            chat_type: ChatType::Private,
            messages: parsed_messages,
            members: vec![],
        })
    }
}

fn parse_skype_timestamp(s: Option<&str>) -> Option<i64> {
    use chrono::DateTime;
    s.and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.timestamp())
}

/// Parser for Google Chat exports.
///
/// Parses Google Chat export files.
pub struct GoogleChatParser {
    name_str: String,
}

impl GoogleChatParser {
    /// Creates a new GoogleChatParser instance.
    pub fn new() -> Self {
        Self {
            name_str: "googlechat".to_string(),
        }
    }
}

impl ChatParser for GoogleChatParser {
    fn name(&self) -> &str {
        &self.name_str
    }

    fn can_parse(&self, path: &Path) -> bool {
        path.to_string_lossy().to_lowercase().contains("google")
            || path.to_string_lossy().to_lowercase().contains("hangouts")
    }

    fn parse(&self, path: &Path) -> Result<ParsedChat, ParseError> {
        let content = std::fs::read_to_string(path)?;

        #[derive(Deserialize)]
        struct GoogleChatMessage {
            sender: Option<GoogleChatSender>,
            create_time: Option<String>,
            text: Option<String>,
        }

        #[derive(Deserialize)]
        struct GoogleChatSender {
            name: Option<String>,
            display_name: Option<String>,
        }

        let messages: Vec<GoogleChatMessage> =
            serde_json::from_str(&content).unwrap_or_else(|_| vec![]);

        let parsed_messages: Vec<ParsedMessage> = messages
            .iter()
            .filter_map(|msg| {
                let sender = msg.sender.as_ref()?;
                let sender_name = sender.display_name.clone().or(sender.name.clone())?;
                let content = msg.text.clone()?;
                if content.is_empty() {
                    return None;
                }

                Some(ParsedMessage {
                    sender: sender_name.clone(),
                    sender_name: Some(sender_name),
                    timestamp: parse_googlechat_timestamp(msg.create_time.as_deref()).unwrap_or(0),
                    content,
                    msg_type: MessageType::Text,
                })
            })
            .collect();

        Ok(ParsedChat {
            platform: "googlechat".to_string(),
            chat_name: path
                .file_stem()
                .ok_or(ParseError::InvalidFormat("missing file stem".to_string()))?
                .to_string_lossy()
                .to_string(),
            chat_type: ChatType::Group,
            messages: parsed_messages,
            members: vec![],
        })
    }
}

fn parse_googlechat_timestamp(s: Option<&str>) -> Option<i64> {
    use chrono::DateTime;
    s.and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.timestamp())
}

/// Parser for Zoom chat exports.
///
/// Parses Zoom chat export files.
pub struct ZoomParser {
    name_str: String,
}

impl ZoomParser {
    /// Creates a new ZoomParser instance.
    pub fn new() -> Self {
        Self {
            name_str: "zoom".to_string(),
        }
    }
}

impl ChatParser for ZoomParser {
    fn name(&self) -> &str {
        &self.name_str
    }

    fn can_parse(&self, path: &Path) -> bool {
        path.to_string_lossy().to_lowercase().contains("zoom")
    }

    fn parse(&self, path: &Path) -> Result<ParsedChat, ParseError> {
        let content = std::fs::read_to_string(path)?;

        #[derive(Deserialize)]
        struct ZoomMessage {
            sender: Option<String>,
            timestamp: Option<String>,
            message: Option<String>,
        }

        let messages: Vec<ZoomMessage> = serde_json::from_str(&content).unwrap_or_else(|_| vec![]);

        let parsed_messages: Vec<ParsedMessage> = messages
            .iter()
            .filter_map(|msg| {
                let sender = msg.sender.clone()?;
                let content = msg.message.clone()?;
                if content.is_empty() {
                    return None;
                }

                Some(ParsedMessage {
                    sender,
                    sender_name: None,
                    timestamp: parse_zoom_timestamp(msg.timestamp.as_deref()).unwrap_or(0),
                    content,
                    msg_type: MessageType::Text,
                })
            })
            .collect();

        Ok(ParsedChat {
            platform: "zoom".to_string(),
            chat_name: path
                .file_stem()
                .ok_or(ParseError::InvalidFormat("missing file stem".to_string()))?
                .to_string_lossy()
                .to_string(),
            chat_type: ChatType::Group,
            messages: parsed_messages,
            members: vec![],
        })
    }
}

fn parse_zoom_timestamp(s: Option<&str>) -> Option<i64> {
    use chrono::DateTime;
    s.and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.timestamp())
}

/// Parser for Viber chat exports.
///
/// Parses Viber chat export files.
pub struct ViberParser {
    name_str: String,
}

impl ViberParser {
    /// Creates a new ViberParser instance.
    pub fn new() -> Self {
        Self {
            name_str: "viber".to_string(),
        }
    }
}

impl ChatParser for ViberParser {
    fn name(&self) -> &str {
        &self.name_str
    }

    fn can_parse(&self, path: &Path) -> bool {
        path.to_string_lossy().to_lowercase().contains("viber")
    }

    fn parse(&self, path: &Path) -> Result<ParsedChat, ParseError> {
        let content = std::fs::read_to_string(path)?;

        #[derive(Deserialize)]
        struct ViberMessage {
            sender: Option<String>,
            date_time: Option<String>,
            text: Option<String>,
        }

        let messages: Vec<ViberMessage> = serde_json::from_str(&content).unwrap_or_else(|_| vec![]);

        let parsed_messages: Vec<ParsedMessage> = messages
            .iter()
            .filter_map(|msg| {
                let sender = msg.sender.clone()?;
                let content = msg.text.clone()?;
                if content.is_empty() {
                    return None;
                }

                Some(ParsedMessage {
                    sender,
                    sender_name: None,
                    timestamp: parse_viber_timestamp(msg.date_time.as_deref()).unwrap_or(0),
                    content,
                    msg_type: MessageType::Text,
                })
            })
            .collect();

        Ok(ParsedChat {
            platform: "viber".to_string(),
            chat_name: path
                .file_stem()
                .ok_or(ParseError::InvalidFormat("missing file stem".to_string()))?
                .to_string_lossy()
                .to_string(),
            chat_type: ChatType::Private,
            messages: parsed_messages,
            members: vec![],
        })
    }
}

fn parse_viber_timestamp(s: Option<&str>) -> Option<i64> {
    use chrono::DateTime;
    s.and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.timestamp())
}

macro_rules! impl_default_parser_via_new {
    ($($parser:ty),+ $(,)?) => {
        $(
            impl Default for $parser {
                fn default() -> Self {
                    Self::new()
                }
            }
        )+
    };
}

impl_default_parser_via_new!(
    WhatsAppParser,
    LINEParser,
    QQParser,
    TelegramParser,
    DiscordParser,
    WeChatParser,
    InstagramParser,
    IMessageParser,
    MessengerParser,
    KakaoTalkParser,
    SlackParser,
    TeamsParser,
    SignalParser,
    SkypeParser,
    GoogleChatParser,
    ZoomParser,
    ViberParser,
);

#[cfg(test)]
mod tests {
    use super::{ParseError, ParserRegistry};
    use std::collections::HashSet;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_FILE_SEQ: AtomicU64 = AtomicU64::new(0);

    fn write_temp_file(prefix: &str, extension: &str, content: &str) -> std::path::PathBuf {
        let epoch_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let seq = TEST_FILE_SEQ.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "xenobot_parser_{prefix}_{epoch_nanos}_{seq}.{extension}"
        ));
        std::fs::write(&path, content).expect("write temp parser fixture");
        path
    }

    #[test]
    fn registry_contains_mainstream_global_platform_parsers() {
        let registry = ParserRegistry::new();
        let names: HashSet<String> = registry.parser_names().into_iter().collect();

        let expected = [
            "whatsapp",
            "line",
            "qq",
            "telegram",
            "discord",
            "wechat",
            "instagram",
            "imessage",
            "messenger",
            "kakaotalk",
            "slack",
            "teams",
            "signal",
            "skype",
            "googlechat",
            "zoom",
            "viber",
        ];

        for parser_name in expected {
            assert!(
                names.contains(parser_name),
                "missing parser in registry: {}",
                parser_name
            );
        }
        assert!(
            registry.parser_count() >= expected.len(),
            "parser count should be at least {}",
            expected.len()
        );
    }

    #[test]
    fn detect_and_parse_uses_content_sniff_when_path_hint_is_missing() {
        let registry = ParserRegistry::new();
        let fixture = write_temp_file(
            "generic_export",
            "txt",
            "2025/03/01 08:00:00 Alice hello from line",
        );

        let parsed = registry
            .detect_and_parse(&fixture)
            .expect("line-formatted content should be detected");
        assert_eq!(parsed.platform, "line");
        assert_eq!(parsed.messages.len(), 1);
        assert_eq!(parsed.messages[0].sender, "Alice");

        let _ = std::fs::remove_file(&fixture);
    }

    #[test]
    fn detect_and_parse_rejects_unrecognized_content_without_hints() {
        let registry = ParserRegistry::new();
        let fixture = write_temp_file(
            "unknown_export",
            "log",
            "this content does not match supported chat export structures",
        );

        let err = registry
            .detect_and_parse(&fixture)
            .expect_err("unknown format should not produce a false parser match");
        assert!(matches!(err, ParseError::UnsupportedFormat(_)));

        let _ = std::fs::remove_file(&fixture);
    }

    #[test]
    fn detect_and_parse_keeps_hinted_empty_export_as_fallback() {
        let registry = ParserRegistry::new();
        let fixture = write_temp_file("whatsapp_empty_export", "txt", "");

        let parsed = registry
            .detect_and_parse(&fixture)
            .expect("hinted empty export should keep deterministic fallback");
        assert_eq!(parsed.platform, "whatsapp");
        assert_eq!(parsed.messages.len(), 0);

        let _ = std::fs::remove_file(&fixture);
    }

    #[test]
    fn detect_and_parse_supports_manual_review_selection_packs() {
        let registry = ParserRegistry::new();
        let fixture = write_temp_file(
            "manual_review_selection",
            "json",
            r#"{
                "schema": "xenobot/manual-review",
                "schemaVersion": 1,
                "captureMode": "manual-selection",
                "platform": "wechat",
                "chatName": "Selected Study Notes",
                "chatType": "group",
                "members": [
                    {"id": "alice", "name": "Alice", "displayName": "Alice"},
                    {"id": "bob", "name": "Bob", "displayName": "Bob"}
                ],
                "selectedMessages": [
                    {
                        "senderId": "alice",
                        "senderName": "Alice",
                        "timestamp": "2025-01-02T10:20:30Z",
                        "content": "Keep this explanation",
                        "messageType": "text"
                    }
                ],
                "selectedFiles": [
                    {
                        "senderId": "bob",
                        "senderName": "Bob",
                        "timestamp": 1735813290,
                        "kind": "image",
                        "path": "screenshots/review-1.png",
                        "note": "Important chart"
                    }
                ]
            }"#,
        );

        let parsed = registry
            .detect_and_parse(&fixture)
            .expect("manual review selection should be detected");
        assert_eq!(parsed.platform, "wechat");
        assert_eq!(parsed.chat_name, "Selected Study Notes");
        assert_eq!(parsed.messages.len(), 2);
        assert_eq!(parsed.members.len(), 2);
        assert!(parsed
            .messages
            .iter()
            .any(|msg| msg.content.contains("Keep this explanation")));
        assert!(parsed
            .messages
            .iter()
            .any(|msg| msg.content.contains("Important chart")));

        let _ = std::fs::remove_file(&fixture);
    }

    #[test]
    fn detect_and_parse_supports_all_17_platform_fixture_shapes() {
        let registry = ParserRegistry::new();
        let fixtures = vec![
            (
                "whatsapp_fixture",
                "txt",
                "[01/02/2025, 10:20:30] Alice: hello whatsapp",
                "whatsapp",
            ),
            (
                "line_fixture",
                "txt",
                "2025/01/02 10:20:30 Alice hello line",
                "line",
            ),
            (
                "qq_fixture",
                "txt",
                "消息记录（此消息记录为文本格式，不支持重新导入）\n消息对象:Release Bridge Group\n2025-01-02 10:20:30 【管理员】Alice(10001)\nhello qq\nwith multiline body\n2025-01-02 10:21:05 Bob<bot@example.com>\n[图片]",
                "qq",
            ),
            (
                "telegram_fixture",
                "json",
                r#"{"name":"tg","messages":[{"from":"Alice","date":"2025-01-02T10:20:30Z","text":"hello telegram"}]}"#,
                "telegram",
            ),
            (
                "discord_fixture",
                "json",
                r#"{"guild":{"id":"g1","name":"Launch Guild"},"channel":{"id":"c1","type":"GuildTextChat","name":"release-war-room"},"messages":[{"id":"1","type":"Default","timestamp":"2025-01-02T10:20:30Z","author":{"id":"u1","name":"Alice","nickname":"Alice"},"content":"hello discord","attachments":[{"id":"a1","fileName":"diagram.png"}],"embeds":[{"title":"Launch checklist"}],"stickers":[{"id":"s1","name":"Ready"}]},{"id":"2","type":"ChannelPinnedMessage","timestamp":"2025-01-02T10:21:30Z","author":{"id":"u2","name":"Bob"},"content":""}]}"#,
                "discord",
            ),
            (
                "wechat_fixture",
                "json",
                r#"{"weflow":{"version":"1.0.0"},"session":{"wxid":"launch-room@chatroom","nickname":"Launch Room","remark":"","displayName":"Launch Room","type":"群聊"},"messages":[{"localId":1,"createTime":1735813230,"type":"文本消息","content":"hello wechat","isSend":0,"senderUsername":"wxid_alice","senderDisplayName":"Alice"},{"localId":2,"createTime":1735813290,"type":"系统消息","content":"Bob joined the room","isSend":null,"senderUsername":"system","senderDisplayName":"System"}]}"#,
                "wechat",
            ),
            (
                "instagram_fixture",
                "json",
                r#"[{"sender":"Alice","timestamp":1735813230,"content":"hello instagram"}]"#,
                "instagram",
            ),
            (
                "imessage_fixture",
                "json",
                r#"[{"text":"hello imessage","sender":"Alice","date":"2025-01-02T10:20:30Z"}]"#,
                "imessage",
            ),
            (
                "messenger_fixture",
                "json",
                r#"[{"sender_name":"Alice","timestamp_ms":1735813230000,"content":"hello messenger"}]"#,
                "messenger",
            ),
            (
                "kakaotalk_fixture",
                "json",
                r#"[{"sender":"Alice","message":"hello kakao","date":"2025-01-02 10:20:30"}]"#,
                "kakaotalk",
            ),
            (
                "slack_fixture",
                "json",
                r#"[{"user":"U1","ts":"1735813230.000200","text":"hello slack"}]"#,
                "slack",
            ),
            (
                "teams_fixture",
                "json",
                r#"[{"from":"Alice","date":"2025-01-02T10:20:30Z","content":"hello teams"}]"#,
                "teams",
            ),
            (
                "signal_fixture",
                "json",
                r#"[{"sender":"Alice","timestamp":1735813230000,"body":"hello signal"}]"#,
                "signal",
            ),
            (
                "skype_fixture",
                "json",
                r#"[{"sender":"Alice","datetime":"2025-01-02T10:20:30Z","msg_content":"hello skype"}]"#,
                "skype",
            ),
            (
                "googlechat_fixture",
                "json",
                r#"[{"sender":{"name":"users/1","display_name":"Alice"},"create_time":"2025-01-02T10:20:30Z","text":"hello googlechat"}]"#,
                "googlechat",
            ),
            (
                "zoom_fixture",
                "json",
                r#"[{"sender":"Alice","timestamp":"2025-01-02T10:20:30Z","message":"hello zoom"}]"#,
                "zoom",
            ),
            (
                "viber_fixture",
                "json",
                r#"[{"sender":"Alice","date_time":"2025-01-02T10:20:30Z","text":"hello viber"}]"#,
                "viber",
            ),
        ];

        for (prefix, ext, content, expected_platform) in fixtures {
            let fixture = write_temp_file(prefix, ext, content);
            let parsed = registry
                .detect_and_parse(&fixture)
                .unwrap_or_else(|e| panic!("fixture '{}' parse failed: {}", prefix, e));
            assert_eq!(
                parsed.platform, expected_platform,
                "platform mismatch for fixture '{}'",
                prefix
            );
            assert!(
                !parsed.messages.is_empty(),
                "fixture '{}' should produce at least one message",
                prefix
            );
            let _ = std::fs::remove_file(&fixture);
        }
    }

    #[test]
    fn qq_parser_supports_official_multiline_export_and_cleans_prefixed_nicknames() {
        let registry = ParserRegistry::new();
        let fixture = write_temp_file(
            "qq_official_export",
            "txt",
            "消息记录（此消息记录为文本格式，不支持重新导入）\n消息对象:Bridge Ops\n2025-01-02 10:20:30 【管理员】Alice(10001)\nhello qq\nthis spans multiple lines\n2025-01-02 10:21:00 Bob<bot@example.com>\n[图片]",
        );

        let parsed = registry
            .detect_and_parse(&fixture)
            .expect("official qq export should parse");
        assert_eq!(parsed.platform, "qq");
        assert_eq!(parsed.chat_name, "Bridge Ops");
        assert_eq!(parsed.messages.len(), 2);
        assert_eq!(parsed.messages[0].sender, "10001");
        assert_eq!(parsed.messages[0].sender_name.as_deref(), Some("Alice"));
        assert_eq!(
            parsed.messages[0].content,
            "hello qq\nthis spans multiple lines"
        );
        assert!(matches!(
            parsed.messages[1].msg_type,
            super::MessageType::Image
        ));
        assert_eq!(parsed.members.len(), 2);
        let _ = std::fs::remove_file(&fixture);
    }

    #[test]
    fn qq_parser_supports_qce_json_export_with_resources_and_sender_profiles() {
        let registry = ParserRegistry::new();
        let fixture = write_temp_file(
            "qq_chat_exporter",
            "json",
            r#"{"chatInfo":{"name":"Release Bridge Group","type":"group","participantCount":3},"statistics":{"totalMessages":2},"messages":[{"id":"msg-1","seq":"1","timestamp":1735813230000,"time":"2025-01-02T10:20:30.000Z","type":"text","recalled":false,"system":false,"sender":{"uid":"uid-alice","uin":"10001","name":"Alice Account","nickname":"Alice","groupCard":"Captain Alice","remark":"Alice Remark"},"content":{"text":"launch checklist is almost done","html":"<p>launch checklist is almost done</p>","elements":[{"type":"text","data":{"text":"launch checklist is almost done"}}],"resources":[{"type":"image","filename":"diagram.png","size":2048}]}},{"id":"msg-2","seq":"2","timestamp":1735813290000,"time":"2025-01-02T10:21:30.000Z","type":"system","recalled":false,"system":true,"sender":{"uid":"uid-bot","name":"System Bot"},"content":{"text":"","html":"","elements":[],"resources":[]}}]}"#,
        );

        let parsed = registry
            .detect_and_parse(&fixture)
            .expect("qce json export should parse");
        assert_eq!(parsed.platform, "qq");
        assert_eq!(parsed.chat_name, "Release Bridge Group");
        assert!(matches!(parsed.chat_type, super::ChatType::Group));
        assert_eq!(parsed.messages.len(), 2);
        assert_eq!(parsed.messages[0].sender, "uid-alice");
        assert_eq!(
            parsed.messages[0].sender_name.as_deref(),
            Some("Captain Alice")
        );
        assert!(parsed.messages[0].content.contains("launch checklist"));
        assert!(parsed.messages[0].content.contains("[Image: diagram.png]"));
        assert!(matches!(
            parsed.messages[0].msg_type,
            super::MessageType::Image
        ));
        let alice = parsed
            .members
            .iter()
            .find(|member| member.id == "uid-alice")
            .expect("qq member should exist");
        assert_eq!(alice.name.as_deref(), Some("Alice Account"));
        assert_eq!(alice.display_name.as_deref(), Some("Captain Alice"));
        let _ = std::fs::remove_file(&fixture);
    }

    #[test]
    fn discord_parser_supports_exporter_root_with_attachments_and_system_events() {
        let registry = ParserRegistry::new();
        let fixture = write_temp_file(
            "discord_exporter",
            "json",
            r#"{"guild":{"id":"g1","name":"Launch Guild"},"channel":{"id":"c1","type":"GuildTextChat","name":"release-room"},"messages":[{"id":"1","type":"Default","timestamp":"2025-01-02T10:20:30Z","author":{"id":"u1","name":"alice.user","nickname":"Captain Alice","globalName":"Alice Global"},"content":"status looks good","attachments":[{"id":"a1","fileName":"diagram.png"}],"embeds":[{"title":"Launch checklist"}],"stickers":[{"id":"s1","name":"Ready"}]},{"id":"2","type":"ChannelPinnedMessage","timestamp":"2025-01-02T10:21:00Z","author":{"id":"u2","name":"Bob"},"content":""}]}"#,
        );

        let parsed = registry
            .detect_and_parse(&fixture)
            .expect("discord exporter root should parse");
        assert_eq!(parsed.platform, "discord");
        assert_eq!(parsed.chat_name, "Launch Guild / #release-room");
        assert_eq!(parsed.messages.len(), 2);
        assert_eq!(
            parsed.messages[0].sender_name.as_deref(),
            Some("Captain Alice")
        );
        assert!(parsed.messages[0].content.contains("[Image: diagram.png]"));
        assert!(parsed.messages[0]
            .content
            .contains("[Link: Launch checklist]"));
        assert!(parsed.messages[0].content.contains("[Sticker: Ready]"));
        assert!(matches!(
            parsed.messages[1].msg_type,
            super::MessageType::System
        ));
        let alice = parsed
            .members
            .iter()
            .find(|member| member.id == "u1")
            .expect("discord member should exist");
        assert_eq!(alice.name.as_deref(), Some("alice.user"));
        assert_eq!(alice.display_name.as_deref(), Some("Captain Alice"));
        let _ = std::fs::remove_file(&fixture);
    }

    #[test]
    fn wechat_parser_supports_weflow_root_and_session_metadata() {
        let registry = ParserRegistry::new();
        let fixture = write_temp_file(
            "weflow_export",
            "json",
            r#"{"weflow":{"version":"1.0.0"},"session":{"wxid":"launch-room@chatroom","nickname":"Launch Room","remark":"","displayName":"Launch Room","type":"群聊"},"messages":[{"localId":1,"createTime":1735813230,"type":"文本消息","content":"hello wechat","isSend":0,"senderUsername":"wxid_alice","senderDisplayName":"Alice"},{"localId":2,"createTime":1735813290,"type":"系统消息","content":"Bob joined the room","isSend":null,"senderUsername":"system","senderDisplayName":"System"}]}"#,
        );

        let parsed = registry
            .detect_and_parse(&fixture)
            .expect("weflow root should parse");
        assert_eq!(parsed.platform, "wechat");
        assert_eq!(parsed.chat_name, "Launch Room");
        assert!(matches!(parsed.chat_type, super::ChatType::Group));
        assert_eq!(parsed.messages.len(), 2);
        assert!(matches!(
            parsed.messages[1].msg_type,
            super::MessageType::System
        ));
        let _ = std::fs::remove_file(&fixture);
    }

    #[test]
    fn wechat_parser_supports_weflow_http_api_root_without_session() {
        let registry = ParserRegistry::new();
        let fixture = write_temp_file(
            "weflow_http_api_export",
            "json",
            r#"{"success":true,"talker":"launch-room@chatroom","count":2,"hasMore":false,"messages":[{"localId":1,"serverId":"456","localType":1,"createTime":1738713600,"isSend":0,"senderUsername":"wxid_member","content":"你好","rawContent":"你好","parsedContent":"Launch readiness still looks good."},{"localId":2,"localType":3,"createTime":1738713660,"isSend":0,"senderUsername":"wxid_member","content":"[图片]","parsedContent":"[图片]","mediaType":"image","mediaFileName":"abc123.jpg","mediaLocalPath":"/tmp/weflow/api-media/launch-room/images/abc123.jpg"}]}"#,
        );

        let parsed = registry
            .detect_and_parse(&fixture)
            .expect("weflow http api root should parse");
        assert_eq!(parsed.platform, "wechat");
        assert_eq!(parsed.chat_name, "launch-room@chatroom");
        assert!(matches!(parsed.chat_type, super::ChatType::Group));
        assert_eq!(parsed.messages.len(), 2);
        assert_eq!(
            parsed.messages[0].content,
            "Launch readiness still looks good."
        );
        assert!(matches!(
            parsed.messages[1].msg_type,
            super::MessageType::Image
        ));
        let _ = std::fs::remove_file(&fixture);
    }
}
