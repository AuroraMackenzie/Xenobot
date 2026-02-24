use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;
use tracing::info;

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
        for parser in &self.parsers {
            if parser.can_parse(path) {
                info!("Detected format: {}", parser.name());
                return parser.parse(path);
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
        let mut messages = Vec::new();
        let mut members = std::collections::HashSet::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some(msg) = parse_qq_line(line) {
                members.insert(msg.sender.clone());
                messages.push(msg);
            }
        }

        Ok(ParsedChat {
            platform: "qq".to_string(),
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

fn parse_qq_line(line: &str) -> Option<ParsedMessage> {
    let pattern =
        regex::Regex::new(r"^\[(\d{4}-\d{2}-\d{2}\s+\d{2}:\d{2}:\d{2})\]\s+(.+?)\s+(.*)$").ok()?;
    let caps = pattern.captures(line)?;

    let timestamp_str = caps.get(1)?.as_str();
    let sender = caps.get(2)?.as_str().to_string();
    let content = caps.get(3)?.as_str().to_string();

    Some(ParsedMessage {
        sender,
        sender_name: None,
        timestamp: parse_qq_timestamp(timestamp_str)?,
        content,
        msg_type: MessageType::Text,
    })
}

fn parse_qq_timestamp(s: &str) -> Option<i64> {
    use chrono::{NaiveDateTime, TimeZone, Utc};
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|dt| Utc.from_utc_datetime(&dt).timestamp())
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

        #[derive(Deserialize)]
        struct DiscordMessage {
            #[serde(rename = "ID")]
            id: Option<String>,
            #[serde(rename = "Timestamp")]
            timestamp: Option<String>,
            #[serde(rename = "Author")]
            author: Option<DiscordAuthor>,
            #[serde(rename = "Content")]
            content: Option<String>,
        }

        #[derive(Deserialize)]
        struct DiscordAuthor {
            #[serde(rename = "ID")]
            id: Option<String>,
            #[serde(rename = "Name")]
            name: Option<String>,
        }

        let messages: Vec<DiscordMessage> = serde_json::from_str(&content)?;

        let parsed_messages: Vec<ParsedMessage> = messages
            .iter()
            .filter_map(|msg| {
                let author = msg.author.as_ref()?;
                let sender = author.id.clone().or(author.name.clone())?;
                let content = msg.content.clone()?;
                if content.is_empty() {
                    return None;
                }

                Some(ParsedMessage {
                    sender,
                    sender_name: author.name.clone(),
                    timestamp: msg
                        .timestamp
                        .as_ref()
                        .and_then(|s| parse_discord_timestamp(s))
                        .unwrap_or(0),
                    content,
                    msg_type: MessageType::Text,
                })
            })
            .collect();

        Ok(ParsedChat {
            platform: "discord".to_string(),
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

fn parse_discord_timestamp(s: &str) -> Option<i64> {
    use chrono::DateTime;
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.timestamp())
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

        #[derive(Deserialize)]
        struct WeFlowMessage {
            #[serde(rename = "msg_id")]
            msg_id: Option<String>,
            #[serde(rename = "type")]
            msg_type: Option<i32>,
            #[serde(rename = "is_sender")]
            is_sender: Option<bool>,
            #[serde(rename = "sender_name")]
            sender_name: Option<String>,
            #[serde(rename = "sender_id")]
            sender_id: Option<String>,
            #[serde(rename = "create_time")]
            create_time: Option<i64>,
            #[serde(rename = "content")]
            content: Option<String>,
        }

        let messages: Vec<WeFlowMessage> = serde_json::from_str(&content)?;

        let parsed_messages: Vec<ParsedMessage> = messages
            .iter()
            .filter_map(|msg| {
                let is_sender = msg.is_sender.unwrap_or(true);
                let sender = if is_sender {
                    "Me".to_string()
                } else {
                    msg.sender_id
                        .clone()
                        .or(msg.sender_name.clone())
                        .unwrap_or_else(|| "Unknown".to_string())
                };

                let content = msg.content.clone()?;
                if content.is_empty() {
                    return None;
                }

                Some(ParsedMessage {
                    sender,
                    sender_name: msg.sender_name.clone(),
                    timestamp: msg.create_time.unwrap_or(0),
                    content,
                    msg_type: MessageType::Text,
                })
            })
            .collect();

        Ok(ParsedChat {
            platform: "wechat".to_string(),
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

#[cfg(test)]
mod tests {
    use super::ParserRegistry;
    use std::collections::HashSet;

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
}
