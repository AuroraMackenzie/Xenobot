//! Merge API module for Xenobot HTTP API.
//!
//! Provides HTTP endpoints equivalent to Xenobot's `mergeApi` IPC methods.

use axum::{routing::post, Json, Router};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::{Path, PathBuf},
};
use tokio::sync::Mutex;
use tracing::instrument;

use crate::ApiError;

static PARSE_CACHE: Lazy<Mutex<HashMap<String, CachedParsedFile>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Merge API router.
pub fn router() -> Router {
    Router::new()
        .route("/parse-file-info", post(parse_file_info))
        .route("/check-conflicts", post(check_conflicts))
        .route("/merge-files", post(merge_files))
        .route("/clear-cache", post(clear_cache))
}

// ==================== Request/Response Types ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileParseInfo {
    name: String,   // 群名
    format: String, // 格式名称
    platform: String,
    message_count: u32,
    member_count: u32,
    file_size: Option<u64>, // 文件大小（字节）
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MergeConflict {
    id: String,
    timestamp: i64,
    sender: String,
    content_length1: usize,
    content_length2: usize,
    content1: String,
    content2: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConflictCheckResult {
    conflicts: Vec<MergeConflict>,
    total_messages: u32, // 合并后预计消息数
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum OutputFormatInput {
    String(String),
    Object {
        #[serde(rename = "formatType")]
        format_type: String,
    },
}

impl OutputFormatInput {
    fn as_str(&self) -> &str {
        match self {
            Self::String(v) => v.as_str(),
            Self::Object { format_type } => format_type.as_str(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ConflictChoice {
    Keep1,
    Keep2,
    KeepBoth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConflictResolution {
    id: String,
    resolution: ConflictChoice,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MergeParams {
    file_paths: Vec<String>,
    output_name: String,
    output_dir: Option<String>,
    output_format: Option<OutputFormatInput>,
    #[serde(default)]
    conflict_resolutions: Vec<ConflictResolution>,
    #[serde(default)]
    and_analyze: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MergeResult {
    success: bool,
    output_path: Option<String>,
    session_id: Option<String>,
    error: Option<String>,
}

// ==================== Handler Implementations ====================

#[derive(Debug, Deserialize)]
struct ParseFileInfoRequest {
    file_path: String,
}

#[derive(Debug, Clone)]
struct ParsedMember {
    platform_id: String,
    account_name: Option<String>,
    group_nickname: Option<String>,
}

#[derive(Debug, Clone)]
struct ParsedMessage {
    sender_platform_id: String,
    sender_name: Option<String>,
    ts: i64,
    msg_type: i64,
    content: Option<String>,
    source_order: usize,
    source_name: String,
    origin_index: usize,
}

#[derive(Debug, Clone)]
struct ParsedPayload {
    name: String,
    platform: String,
    chat_type: String,
    members: HashMap<String, ParsedMember>,
    messages: Vec<ParsedMessage>,
}

#[derive(Debug, Clone)]
struct CachedParsedFile {
    info: FileParseInfo,
    payload: ParsedPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct ConflictGroupKey {
    timestamp: i64,
    sender: String,
}

#[derive(Debug, Clone)]
struct ConflictGroupEntry {
    content: String,
    sender_name: Option<String>,
    msg_type: i64,
    source_order: usize,
    source_name: String,
    origin_index: usize,
}

#[derive(Debug)]
struct ConflictComputation {
    conflicts: Vec<MergeConflict>,
    total_messages: u32,
    groups: BTreeMap<ConflictGroupKey, Vec<ConflictGroupEntry>>,
    conflict_pairs: HashMap<String, (ConflictGroupKey, usize, usize)>,
}

fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn infer_platform_from_path(file_path: &str) -> String {
    let lower = file_path.to_ascii_lowercase();
    if lower.contains("telegram") || lower.contains("tg") {
        "telegram".to_string()
    } else if lower.contains("wechat") || lower.contains("wx") {
        "wechat".to_string()
    } else if lower.contains("qq") {
        "qq".to_string()
    } else if lower.contains("line") {
        "line".to_string()
    } else if lower.contains("whatsapp") || lower.contains("wa") {
        "whatsapp".to_string()
    } else if lower.contains("discord") {
        "discord".to_string()
    } else if lower.contains("instagram") || lower.contains("ig") {
        "instagram".to_string()
    } else {
        "generic".to_string()
    }
}

fn file_stem_name(file_path: &str) -> String {
    Path::new(file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|v| v.to_string())
        .unwrap_or_else(|| "Merged Chat".to_string())
}

fn classify_chat_type(raw_type: &str) -> String {
    let t = raw_type.to_ascii_lowercase();
    if t.contains("private") || t.contains("personal") || t.contains("bot") || t.contains("saved") {
        "private".to_string()
    } else {
        "group".to_string()
    }
}

fn as_i64(value: &serde_json::Value) -> Option<i64> {
    match value {
        serde_json::Value::Number(n) => n.as_i64(),
        serde_json::Value::String(s) => s.parse::<i64>().ok(),
        _ => None,
    }
}

fn as_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn extract_text(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        serde_json::Value::Array(arr) => {
            let mut parts = Vec::new();
            for item in arr {
                if let Some(text) = extract_text(item) {
                    parts.push(text);
                }
            }
            if parts.is_empty() {
                None
            } else {
                Some(parts.join(""))
            }
        }
        serde_json::Value::Object(obj) => {
            for key in ["text", "content", "message", "msg", "title"] {
                if let Some(v) = obj.get(key) {
                    if let Some(text) = extract_text(v) {
                        return Some(text);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn normalized_timestamp(raw: i64) -> Option<i64> {
    let mut ts = raw;
    if ts > 10_000_000_000 {
        ts /= 1000;
    }
    if ts <= 0 {
        None
    } else {
        Some(ts)
    }
}

fn parse_message_type(obj: &serde_json::Map<String, serde_json::Value>) -> i64 {
    for key in ["type", "msg_type", "message_type"] {
        if let Some(v) = obj.get(key) {
            if let Some(n) = as_i64(v) {
                return n;
            }
            if let Some(s) = v.as_str() {
                let lower = s.to_ascii_lowercase();
                if lower.contains("text") || lower == "message" {
                    return 0;
                }
                if lower.contains("image") || lower.contains("photo") || lower.contains("sticker") {
                    return 1;
                }
                if lower.contains("voice") || lower.contains("audio") {
                    return 2;
                }
                if lower.contains("video") {
                    return 3;
                }
                if lower.contains("file") || lower.contains("document") {
                    return 4;
                }
                return 0;
            }
        }
    }
    0
}

fn parse_sender(obj: &serde_json::Map<String, serde_json::Value>) -> (String, Option<String>) {
    let mut sender_id: Option<String> = None;
    let mut sender_name: Option<String> = None;

    for id_key in [
        "sender",
        "senderPlatformId",
        "sender_id",
        "from_id",
        "user_id",
        "author_id",
        "platform_id",
    ] {
        if let Some(v) = obj.get(id_key).and_then(as_string) {
            sender_id = Some(v);
            break;
        }
    }

    for name_key in [
        "accountName",
        "senderAccountName",
        "sender_name",
        "sender",
        "from_name",
        "author",
        "name",
        "nickname",
        "groupNickname",
        "senderGroupNickname",
    ] {
        if let Some(v) = obj.get(name_key) {
            if let Some(s) = v.as_str() {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    sender_name = Some(trimmed.to_string());
                    if sender_id.is_none() {
                        sender_id = Some(trimmed.to_string());
                    }
                    break;
                }
            }
        }
    }

    let sender_id = sender_id
        .or_else(|| sender_name.as_ref().map(|v| format!("name:{}", v)))
        .unwrap_or_else(|| "unknown".to_string());

    (sender_id, sender_name)
}

fn parse_message_object(
    obj: &serde_json::Map<String, serde_json::Value>,
    source_order: usize,
    source_name: &str,
    origin_index: usize,
) -> Option<ParsedMessage> {
    let mut ts: Option<i64> = None;
    for key in [
        "timestamp",
        "ts",
        "time",
        "date_unixtime",
        "date",
        "send_time",
        "create_time",
    ] {
        if let Some(raw) = obj.get(key).and_then(as_i64) {
            ts = normalized_timestamp(raw);
            if ts.is_some() {
                break;
            }
        }
    }
    let ts = ts?;

    let (sender_platform_id, sender_name) = parse_sender(obj);
    let msg_type = parse_message_type(obj);
    let content = obj
        .get("content")
        .or_else(|| obj.get("text"))
        .or_else(|| obj.get("message"))
        .or_else(|| obj.get("msg"))
        .and_then(extract_text);

    Some(ParsedMessage {
        sender_platform_id,
        sender_name,
        ts,
        msg_type,
        content,
        source_order,
        source_name: source_name.to_string(),
        origin_index,
    })
}

fn parse_members_from_json(members: &[serde_json::Value], out: &mut HashMap<String, ParsedMember>) {
    for item in members {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let platform_id = obj
            .get("platformId")
            .or_else(|| obj.get("platform_id"))
            .and_then(as_string)
            .unwrap_or_default();
        if platform_id.trim().is_empty() {
            continue;
        }
        let account_name = obj
            .get("accountName")
            .or_else(|| obj.get("account_name"))
            .and_then(as_string);
        let group_nickname = obj
            .get("groupNickname")
            .or_else(|| obj.get("group_nickname"))
            .and_then(as_string);

        out.entry(platform_id.clone())
            .and_modify(|existing| {
                if account_name.is_some() {
                    existing.account_name = account_name.clone();
                }
                if group_nickname.is_some() {
                    existing.group_nickname = group_nickname.clone();
                }
            })
            .or_insert(ParsedMember {
                platform_id,
                account_name,
                group_nickname,
            });
    }
}

fn parse_xenobot_json_object(
    obj: &serde_json::Map<String, serde_json::Value>,
    file_path: &str,
) -> ParsedPayload {
    let mut name = file_stem_name(file_path);
    let mut platform = infer_platform_from_path(file_path);
    let mut chat_type = "group".to_string();
    let mut members: HashMap<String, ParsedMember> = HashMap::new();
    let mut messages: Vec<ParsedMessage> = Vec::new();
    let source_name = Path::new(file_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(file_path);

    if let Some(meta) = obj.get("meta").and_then(|v| v.as_object()) {
        if let Some(v) = meta.get("name").and_then(as_string) {
            name = v;
        }
        if let Some(v) = meta.get("platform").and_then(as_string) {
            platform = v;
        }
        if let Some(v) = meta.get("type").and_then(as_string) {
            chat_type = classify_chat_type(&v);
        }
    } else {
        if let Some(v) = obj
            .get("name")
            .or_else(|| obj.get("title"))
            .and_then(as_string)
        {
            name = v;
        }
        if let Some(v) = obj.get("platform").and_then(as_string) {
            platform = v;
        }
        if let Some(v) = obj.get("type").and_then(as_string) {
            chat_type = classify_chat_type(&v);
        }
    }

    if let Some(member_arr) = obj.get("members").and_then(|v| v.as_array()) {
        parse_members_from_json(member_arr, &mut members);
    }

    if let Some(msg_arr) = obj.get("messages").and_then(|v| v.as_array()) {
        for (idx, item) in msg_arr.iter().enumerate() {
            if let Some(msg_obj) = item.as_object() {
                if let Some(message) = parse_message_object(msg_obj, 0, source_name, idx) {
                    messages.push(message);
                }
            }
        }
    }

    ParsedPayload {
        name,
        platform,
        chat_type,
        members,
        messages,
    }
}

fn parse_json_value(file_path: &str, value: &serde_json::Value) -> ParsedPayload {
    let source_name = Path::new(file_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(file_path);

    if let Some(obj) = value.as_object() {
        if obj.get("messages").is_some()
            || obj.get("meta").is_some()
            || obj.get("members").is_some()
        {
            return parse_xenobot_json_object(obj, file_path);
        }

        if let Some(chats) = obj
            .get("chats")
            .and_then(|v| v.get("list").or_else(|| Some(v)))
            .and_then(|v| v.as_array())
        {
            let mut payload = ParsedPayload {
                name: file_stem_name(file_path),
                platform: infer_platform_from_path(file_path),
                chat_type: "group".to_string(),
                members: HashMap::new(),
                messages: Vec::new(),
            };

            for (chat_idx, chat) in chats.iter().enumerate() {
                let Some(chat_obj) = chat.as_object() else {
                    continue;
                };

                if chat_idx == 0 {
                    if let Some(v) = chat_obj
                        .get("name")
                        .or_else(|| chat_obj.get("title"))
                        .and_then(as_string)
                    {
                        payload.name = v;
                    }
                    if let Some(v) = chat_obj.get("type").and_then(as_string) {
                        payload.chat_type = classify_chat_type(&v);
                    }
                }

                if let Some(arr) = chat_obj.get("messages").and_then(|v| v.as_array()) {
                    for (msg_idx, item) in arr.iter().enumerate() {
                        if let Some(msg_obj) = item.as_object() {
                            if let Some(msg) = parse_message_object(
                                msg_obj,
                                0,
                                source_name,
                                chat_idx * 1_000_000 + msg_idx,
                            ) {
                                payload.messages.push(msg);
                            }
                        }
                    }
                }
            }
            return payload;
        }
    }

    if let Some(obj) = value.as_object() {
        return parse_xenobot_json_object(obj, file_path);
    }

    if let Some(arr) = value.as_array() {
        let mut payload = ParsedPayload {
            name: file_stem_name(file_path),
            platform: infer_platform_from_path(file_path),
            chat_type: "group".to_string(),
            members: HashMap::new(),
            messages: Vec::new(),
        };
        for (idx, item) in arr.iter().enumerate() {
            if let Some(msg_obj) = item.as_object() {
                if let Some(msg) = parse_message_object(msg_obj, 0, source_name, idx) {
                    payload.messages.push(msg);
                }
            }
        }
        return payload;
    }

    ParsedPayload {
        name: file_stem_name(file_path),
        platform: infer_platform_from_path(file_path),
        chat_type: "group".to_string(),
        members: HashMap::new(),
        messages: Vec::new(),
    }
}

fn parse_jsonl(file_path: &str, text: &str) -> ParsedPayload {
    let source_name = Path::new(file_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(file_path);
    let mut payload = ParsedPayload {
        name: file_stem_name(file_path),
        platform: infer_platform_from_path(file_path),
        chat_type: "group".to_string(),
        members: HashMap::new(),
        messages: Vec::new(),
    };

    for (line_idx, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            continue;
        };
        let Some(obj) = value.as_object() else {
            continue;
        };

        if let Some(record_type) = obj.get("_type").and_then(|v| v.as_str()) {
            match record_type {
                "header" => {
                    if let Some(meta) = obj.get("meta").and_then(|v| v.as_object()) {
                        if let Some(v) = meta.get("name").and_then(as_string) {
                            payload.name = v;
                        }
                        if let Some(v) = meta.get("platform").and_then(as_string) {
                            payload.platform = v;
                        }
                        if let Some(v) = meta.get("type").and_then(as_string) {
                            payload.chat_type = classify_chat_type(&v);
                        }
                    }
                }
                "member" => {
                    let platform_id = obj
                        .get("platformId")
                        .or_else(|| obj.get("platform_id"))
                        .and_then(as_string)
                        .unwrap_or_default();
                    if !platform_id.trim().is_empty() {
                        payload.members.insert(
                            platform_id.clone(),
                            ParsedMember {
                                platform_id,
                                account_name: obj
                                    .get("accountName")
                                    .or_else(|| obj.get("account_name"))
                                    .and_then(as_string),
                                group_nickname: obj
                                    .get("groupNickname")
                                    .or_else(|| obj.get("group_nickname"))
                                    .and_then(as_string),
                            },
                        );
                    }
                }
                "message" => {
                    if let Some(msg) = parse_message_object(obj, 0, source_name, line_idx) {
                        payload.messages.push(msg);
                    }
                }
                _ => {
                    if let Some(msg) = parse_message_object(obj, 0, source_name, line_idx) {
                        payload.messages.push(msg);
                    }
                }
            }
        } else if let Some(msg) = parse_message_object(obj, 0, source_name, line_idx) {
            payload.messages.push(msg);
        }
    }

    payload
}

fn parse_text(file_path: &str, text: &str) -> ParsedPayload {
    let mut messages = Vec::new();
    let base_ts = now_ts();
    let source_name = Path::new(file_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(file_path)
        .to_string();

    for (idx, line) in text.lines().enumerate() {
        let content = line.trim();
        if content.is_empty() {
            continue;
        }
        messages.push(ParsedMessage {
            sender_platform_id: "text-importer".to_string(),
            sender_name: Some("文本导入".to_string()),
            ts: base_ts + idx as i64,
            msg_type: 0,
            content: Some(content.to_string()),
            source_order: 0,
            source_name: source_name.clone(),
            origin_index: idx,
        });
    }

    ParsedPayload {
        name: file_stem_name(file_path),
        platform: infer_platform_from_path(file_path),
        chat_type: "group".to_string(),
        members: HashMap::new(),
        messages,
    }
}

fn normalize_output_format(format: Option<&OutputFormatInput>) -> &'static str {
    match format {
        Some(v) if v.as_str().eq_ignore_ascii_case("jsonl") => "jsonl",
        _ => "json",
    }
}

fn sanitize_output_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == ' ' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim();
    if trimmed.is_empty() {
        "merged_chat".to_string()
    } else {
        trimmed.to_string()
    }
}

fn default_output_dir() -> PathBuf {
    dirs::download_dir()
        .or_else(|| dirs::data_dir())
        .unwrap_or_else(std::env::temp_dir)
        .join("xenobot")
}

fn ensure_members_from_messages(payload: &mut ParsedPayload) {
    for msg in &payload.messages {
        payload
            .members
            .entry(msg.sender_platform_id.clone())
            .and_modify(|m| {
                if m.account_name.is_none() && msg.sender_name.is_some() {
                    m.account_name = msg.sender_name.clone();
                }
            })
            .or_insert(ParsedMember {
                platform_id: msg.sender_platform_id.clone(),
                account_name: msg.sender_name.clone(),
                group_nickname: None,
            });
    }
}

fn normalize_content_for_key(content: Option<&str>) -> String {
    content.unwrap_or_default().trim().replace('\n', " ")
}

fn unique_message_key(msg: &ParsedMessage) -> String {
    format!(
        "{}|{}|{}",
        msg.ts,
        msg.sender_platform_id,
        normalize_content_for_key(msg.content.as_deref())
    )
}

fn compute_conflicts(messages: &[ParsedMessage]) -> ConflictComputation {
    let mut ordered = messages.to_vec();
    ordered.sort_by(|a, b| {
        a.ts.cmp(&b.ts)
            .then_with(|| a.source_order.cmp(&b.source_order))
            .then_with(|| a.origin_index.cmp(&b.origin_index))
            .then_with(|| a.sender_platform_id.cmp(&b.sender_platform_id))
    });

    let mut seen = HashSet::new();
    let mut unique = Vec::new();
    for msg in ordered {
        let key = unique_message_key(&msg);
        if seen.insert(key) {
            unique.push(msg);
        }
    }

    let mut groups: BTreeMap<ConflictGroupKey, Vec<ConflictGroupEntry>> = BTreeMap::new();
    for msg in unique {
        let group_key = ConflictGroupKey {
            timestamp: msg.ts,
            sender: msg.sender_platform_id.clone(),
        };
        let content = normalize_content_for_key(msg.content.as_deref());
        let entries = groups.entry(group_key).or_default();
        if entries.iter().any(|existing| existing.content == content) {
            continue;
        }
        entries.push(ConflictGroupEntry {
            content,
            sender_name: msg.sender_name.clone(),
            msg_type: msg.msg_type,
            source_order: msg.source_order,
            source_name: msg.source_name.clone(),
            origin_index: msg.origin_index,
        });
    }

    let mut conflict_idx = 0usize;
    let mut conflicts = Vec::new();
    let mut conflict_pairs = HashMap::new();
    for (group_key, entries) in &groups {
        if entries.len() < 2 {
            continue;
        }
        for i in 0..entries.len() - 1 {
            for j in i + 1..entries.len() {
                let id = format!(
                    "conflict_{}_{}_{}",
                    group_key.timestamp, group_key.sender, conflict_idx
                );
                conflict_idx += 1;

                conflicts.push(MergeConflict {
                    id: id.clone(),
                    timestamp: group_key.timestamp,
                    sender: entries[i]
                        .sender_name
                        .clone()
                        .unwrap_or_else(|| group_key.sender.clone()),
                    content_length1: entries[i].content.chars().count(),
                    content_length2: entries[j].content.chars().count(),
                    content1: entries[i].content.clone(),
                    content2: entries[j].content.clone(),
                });
                conflict_pairs.insert(id, (group_key.clone(), i, j));
            }
        }
    }

    let total_messages: u32 = groups.values().map(|v| v.len() as u32).sum();
    ConflictComputation {
        conflicts,
        total_messages,
        groups,
        conflict_pairs,
    }
}

async fn parse_file_uncached(file_path: &str) -> Result<CachedParsedFile, ApiError> {
    let metadata = tokio::fs::metadata(file_path)
        .await
        .map_err(|_| ApiError::InvalidRequest(format!("File not found: {}", file_path)))?;
    let file_size = metadata.len();
    let bytes = tokio::fs::read(file_path)
        .await
        .map_err(|e| ApiError::Io(e))?;
    let ext = Path::new(file_path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let text = String::from_utf8_lossy(&bytes);

    let mut payload = if ext == "jsonl" {
        parse_jsonl(file_path, &text)
    } else if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
        parse_json_value(file_path, &value)
    } else {
        parse_text(file_path, &text)
    };

    ensure_members_from_messages(&mut payload);

    let format_name = if ext == "jsonl" {
        "jsonl".to_string()
    } else if ext == "json" {
        "json".to_string()
    } else if serde_json::from_slice::<serde_json::Value>(&bytes).is_ok() {
        "json".to_string()
    } else {
        "text".to_string()
    };

    let info = FileParseInfo {
        name: payload.name.clone(),
        format: format_name,
        platform: payload.platform.clone(),
        message_count: payload.messages.len() as u32,
        member_count: payload.members.len() as u32,
        file_size: Some(file_size),
    };

    Ok(CachedParsedFile { info, payload })
}

async fn get_or_parse_file(file_path: &str) -> Result<CachedParsedFile, ApiError> {
    {
        let cache = PARSE_CACHE.lock().await;
        if let Some(cached) = cache.get(file_path) {
            return Ok(cached.clone());
        }
    }

    let parsed = parse_file_uncached(file_path).await?;
    let mut cache = PARSE_CACHE.lock().await;
    cache.insert(file_path.to_string(), parsed.clone());
    Ok(parsed)
}

async fn merge_into_database(
    output_name: &str,
    merged_platform: &str,
    merged_chat_type: &str,
    members: &[ParsedMember],
    messages: &[ParsedMessage],
) -> Result<String, ApiError> {
    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    let repo = crate::database::Repository::new(pool.clone());

    let imported_at = now_ts();
    let meta_id = repo
        .create_chat(&crate::database::repository::ChatMeta {
            id: 0,
            name: output_name.to_string(),
            platform: merged_platform.to_string(),
            chat_type: merged_chat_type.to_string(),
            imported_at,
            group_id: None,
            group_avatar: None,
            owner_id: None,
            schema_version: 3,
            session_gap_threshold: 1800,
        })
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let mut member_id_map: HashMap<String, i64> = HashMap::new();
    for member in members {
        let account_name = member
            .account_name
            .as_deref()
            .or(member.group_nickname.as_deref());
        let member_id = repo
            .get_or_create_member(&member.platform_id, account_name)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;
        member_id_map.insert(member.platform_id.clone(), member_id);
    }

    for (idx, msg) in messages.iter().enumerate() {
        let sender_id = if let Some(id) = member_id_map.get(&msg.sender_platform_id) {
            *id
        } else {
            let created = repo
                .get_or_create_member(&msg.sender_platform_id, msg.sender_name.as_deref())
                .await
                .map_err(|e| ApiError::Database(e.to_string()))?;
            member_id_map.insert(msg.sender_platform_id.clone(), created);
            created
        };

        repo.create_message(&crate::database::repository::Message {
            id: 0,
            sender_id,
            sender_account_name: msg.sender_name.clone(),
            sender_group_nickname: msg.sender_name.clone(),
            ts: if msg.ts > 0 {
                msg.ts
            } else {
                imported_at + idx as i64
            },
            msg_type: msg.msg_type,
            content: msg.content.clone(),
            reply_to_message_id: None,
            platform_message_id: None,
            meta_id,
        })
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    }

    Ok(meta_id.to_string())
}

#[instrument]
async fn parse_file_info(
    Json(req): Json<ParseFileInfoRequest>,
) -> Result<Json<FileParseInfo>, ApiError> {
    let parsed = get_or_parse_file(&req.file_path).await?;
    Ok(Json(parsed.info))
}

#[derive(Debug, Deserialize)]
struct CheckConflictsRequest {
    file_paths: Vec<String>,
}

#[instrument]
async fn check_conflicts(
    Json(req): Json<CheckConflictsRequest>,
) -> Result<Json<ConflictCheckResult>, ApiError> {
    if req.file_paths.is_empty() {
        return Err(ApiError::InvalidRequest(
            "filePaths cannot be empty".to_string(),
        ));
    }

    let mut all_messages = Vec::new();
    let mut platforms = HashSet::new();
    for (source_order, file_path) in req.file_paths.iter().enumerate() {
        let parsed = get_or_parse_file(file_path).await?;
        platforms.insert(parsed.payload.platform.clone());
        let source_name = Path::new(file_path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(file_path);

        for (idx, msg) in parsed.payload.messages.iter().enumerate() {
            let mut cloned = msg.clone();
            cloned.source_order = source_order;
            cloned.source_name = source_name.to_string();
            cloned.origin_index = idx;
            all_messages.push(cloned);
        }
    }

    if platforms.len() > 1 {
        return Err(ApiError::InvalidRequest(
            "Merging different platforms in one merge is not supported".to_string(),
        ));
    }

    let computed = compute_conflicts(&all_messages);
    Ok(Json(ConflictCheckResult {
        conflicts: computed.conflicts,
        total_messages: computed.total_messages,
    }))
}

#[instrument]
async fn merge_files(Json(req): Json<MergeParams>) -> Result<Json<MergeResult>, ApiError> {
    if req.file_paths.len() < 2 {
        return Ok(Json(MergeResult {
            success: false,
            output_path: None,
            session_id: None,
            error: Some("At least two files are required".to_string()),
        }));
    }
    if req.output_name.trim().is_empty() {
        return Ok(Json(MergeResult {
            success: false,
            output_path: None,
            session_id: None,
            error: Some("outputName cannot be empty".to_string()),
        }));
    }

    let mut parse_results = Vec::new();
    let mut all_messages = Vec::new();
    let mut platforms = HashSet::new();

    for (source_order, file_path) in req.file_paths.iter().enumerate() {
        let parsed = match get_or_parse_file(file_path).await {
            Ok(v) => v,
            Err(e) => {
                return Ok(Json(MergeResult {
                    success: false,
                    output_path: None,
                    session_id: None,
                    error: Some(e.to_string()),
                }))
            }
        };
        platforms.insert(parsed.payload.platform.clone());

        let source_name = Path::new(file_path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(file_path)
            .to_string();
        let mut payload = parsed.payload.clone();
        for (idx, msg) in payload.messages.iter_mut().enumerate() {
            msg.source_order = source_order;
            msg.source_name = source_name.clone();
            msg.origin_index = idx;
            all_messages.push(msg.clone());
        }
        parse_results.push((file_path.clone(), parsed.info.clone(), payload));
    }

    if platforms.len() > 1 {
        return Ok(Json(MergeResult {
            success: false,
            output_path: None,
            session_id: None,
            error: Some("Merging different platforms in one merge is not supported".to_string()),
        }));
    }

    let computed = compute_conflicts(&all_messages);
    let mut resolution_map: HashMap<String, ConflictChoice> = HashMap::new();
    for item in &req.conflict_resolutions {
        resolution_map.insert(item.id.clone(), item.resolution.clone());
    }

    let mut group_decisions: HashMap<ConflictGroupKey, usize> = HashMap::new();
    for (conflict_id, (group_key, i, j)) in &computed.conflict_pairs {
        let Some(choice) = resolution_map.get(conflict_id) else {
            continue;
        };
        match choice {
            ConflictChoice::Keep1 => {
                group_decisions.entry(group_key.clone()).or_insert(*i);
            }
            ConflictChoice::Keep2 => {
                group_decisions.entry(group_key.clone()).or_insert(*j);
            }
            ConflictChoice::KeepBoth => {}
        }
    }

    let mut selected_messages = Vec::new();
    for (group_key, entries) in &computed.groups {
        if let Some(idx) = group_decisions.get(group_key).copied() {
            if let Some(entry) = entries.get(idx) {
                selected_messages.push(ParsedMessage {
                    sender_platform_id: group_key.sender.clone(),
                    sender_name: entry.sender_name.clone(),
                    ts: group_key.timestamp,
                    msg_type: entry.msg_type,
                    content: if entry.content.is_empty() {
                        None
                    } else {
                        Some(entry.content.clone())
                    },
                    source_order: entry.source_order,
                    source_name: entry.source_name.clone(),
                    origin_index: entry.origin_index,
                });
            }
        } else {
            for entry in entries {
                selected_messages.push(ParsedMessage {
                    sender_platform_id: group_key.sender.clone(),
                    sender_name: entry.sender_name.clone(),
                    ts: group_key.timestamp,
                    msg_type: entry.msg_type,
                    content: if entry.content.is_empty() {
                        None
                    } else {
                        Some(entry.content.clone())
                    },
                    source_order: entry.source_order,
                    source_name: entry.source_name.clone(),
                    origin_index: entry.origin_index,
                });
            }
        }
    }

    selected_messages.sort_by(|a, b| {
        a.ts.cmp(&b.ts)
            .then_with(|| a.source_order.cmp(&b.source_order))
            .then_with(|| a.origin_index.cmp(&b.origin_index))
    });

    let mut merged_members: HashMap<String, ParsedMember> = HashMap::new();
    for (_, _, payload) in &parse_results {
        for (platform_id, member) in &payload.members {
            merged_members
                .entry(platform_id.clone())
                .and_modify(|existing| {
                    if member.account_name.is_some() {
                        existing.account_name = member.account_name.clone();
                    }
                    if member.group_nickname.is_some() {
                        existing.group_nickname = member.group_nickname.clone();
                    }
                })
                .or_insert_with(|| member.clone());
        }
    }
    for msg in &selected_messages {
        merged_members
            .entry(msg.sender_platform_id.clone())
            .or_insert(ParsedMember {
                platform_id: msg.sender_platform_id.clone(),
                account_name: msg.sender_name.clone(),
                group_nickname: None,
            });
    }

    let mut merged_members_vec: Vec<ParsedMember> = merged_members.into_values().collect();
    merged_members_vec.sort_by(|a, b| a.platform_id.cmp(&b.platform_id));

    let merged_platform = platforms
        .iter()
        .next()
        .cloned()
        .unwrap_or_else(|| "generic".to_string());
    let merged_chat_type = parse_results
        .first()
        .map(|(_, _, p)| p.chat_type.clone())
        .unwrap_or_else(|| "group".to_string());
    let output_format = normalize_output_format(req.output_format.as_ref());

    let output_dir = req
        .output_dir
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(default_output_dir);
    if let Err(e) = tokio::fs::create_dir_all(&output_dir).await {
        return Ok(Json(MergeResult {
            success: false,
            output_path: None,
            session_id: None,
            error: Some(format!("Failed to create output dir: {}", e)),
        }));
    }

    let date = chrono::Utc::now().format("%Y%m%d").to_string();
    let file_name = format!(
        "{}_merged_{}.{}",
        sanitize_output_name(&req.output_name),
        date,
        output_format
    );
    let output_path = output_dir.join(file_name);

    let sources_json: Vec<serde_json::Value> = parse_results
        .iter()
        .map(|(path, info, payload)| {
            serde_json::json!({
                "filename": Path::new(path).file_name().and_then(|s| s.to_str()).unwrap_or(path),
                "platform": payload.platform,
                "messageCount": info.message_count
            })
        })
        .collect();

    let members_json: Vec<serde_json::Value> = merged_members_vec
        .iter()
        .map(|m| {
            serde_json::json!({
                "platformId": m.platform_id,
                "accountName": m.account_name,
                "groupNickname": m.group_nickname
            })
        })
        .collect();
    let messages_json: Vec<serde_json::Value> = selected_messages
        .iter()
        .map(|m| {
            serde_json::json!({
                "sender": m.sender_platform_id,
                "accountName": m.sender_name,
                "groupNickname": serde_json::Value::Null,
                "timestamp": m.ts,
                "type": m.msg_type,
                "content": m.content
            })
        })
        .collect();

    let header_json = serde_json::json!({
        "version": "0.0.1",
        "exportedAt": now_ts(),
        "generator": "Xenobot Merge Tool",
        "description": format!("Merged from {} files", parse_results.len())
    });
    let meta_json = serde_json::json!({
        "name": req.output_name,
        "platform": merged_platform,
        "type": merged_chat_type,
        "sources": sources_json
    });

    let write_result: Result<(), ApiError> = if output_format == "jsonl" {
        let mut lines = String::new();
        lines.push_str(
            &serde_json::json!({
                "_type": "header",
                "xenobot": header_json,
                "meta": meta_json
            })
            .to_string(),
        );
        lines.push('\n');
        for member in &members_json {
            lines.push_str(
                &serde_json::json!({
                    "_type": "member",
                    "platformId": member.get("platformId").cloned().unwrap_or(serde_json::Value::Null),
                    "accountName": member.get("accountName").cloned().unwrap_or(serde_json::Value::Null),
                    "groupNickname": member.get("groupNickname").cloned().unwrap_or(serde_json::Value::Null)
                })
                .to_string(),
            );
            lines.push('\n');
        }
        for msg in &messages_json {
            lines.push_str(
                &serde_json::json!({
                    "_type": "message",
                    "sender": msg.get("sender").cloned().unwrap_or(serde_json::Value::Null),
                    "accountName": msg.get("accountName").cloned().unwrap_or(serde_json::Value::Null),
                    "groupNickname": msg.get("groupNickname").cloned().unwrap_or(serde_json::Value::Null),
                    "timestamp": msg.get("timestamp").cloned().unwrap_or(serde_json::Value::Null),
                    "type": msg.get("type").cloned().unwrap_or(serde_json::Value::Null),
                    "content": msg.get("content").cloned().unwrap_or(serde_json::Value::Null)
                })
                .to_string(),
            );
            lines.push('\n');
        }
        tokio::fs::write(&output_path, lines)
            .await
            .map_err(ApiError::Io)
    } else {
        let merged_json = serde_json::json!({
            "xenobot": header_json,
            "meta": meta_json,
            "members": members_json,
            "messages": messages_json
        });
        let bytes = serde_json::to_vec_pretty(&merged_json).map_err(ApiError::Json)?;
        tokio::fs::write(&output_path, bytes)
            .await
            .map_err(ApiError::Io)
    };

    if let Err(e) = write_result {
        return Ok(Json(MergeResult {
            success: false,
            output_path: None,
            session_id: None,
            error: Some(e.to_string()),
        }));
    }

    let mut session_id = None;
    if req.and_analyze {
        match merge_into_database(
            &req.output_name,
            &merged_platform,
            &merged_chat_type,
            &merged_members_vec,
            &selected_messages,
        )
        .await
        {
            Ok(id) => session_id = Some(id),
            Err(e) => {
                return Ok(Json(MergeResult {
                    success: false,
                    output_path: Some(output_path.to_string_lossy().to_string()),
                    session_id: None,
                    error: Some(e.to_string()),
                }));
            }
        }
    }

    Ok(Json(MergeResult {
        success: true,
        output_path: Some(output_path.to_string_lossy().to_string()),
        session_id,
        error: None,
    }))
}

#[derive(Debug, Deserialize)]
struct ClearCacheRequest {
    file_path: Option<String>,
}

#[instrument]
async fn clear_cache(Json(req): Json<ClearCacheRequest>) -> Result<Json<bool>, ApiError> {
    let mut cache = PARSE_CACHE.lock().await;
    if let Some(path) = req.file_path {
        cache.remove(&path);
    } else {
        cache.clear();
    }
    Ok(Json(true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_conflicts_detects_conflict_and_dedup() {
        let messages = vec![
            ParsedMessage {
                sender_platform_id: "u1".to_string(),
                sender_name: Some("Alice".to_string()),
                ts: 100,
                msg_type: 0,
                content: Some("hello".to_string()),
                source_order: 0,
                source_name: "a.json".to_string(),
                origin_index: 0,
            },
            ParsedMessage {
                sender_platform_id: "u1".to_string(),
                sender_name: Some("Alice".to_string()),
                ts: 100,
                msg_type: 0,
                content: Some("hello".to_string()),
                source_order: 1,
                source_name: "b.json".to_string(),
                origin_index: 0,
            },
            ParsedMessage {
                sender_platform_id: "u1".to_string(),
                sender_name: Some("Alice".to_string()),
                ts: 100,
                msg_type: 0,
                content: Some("different".to_string()),
                source_order: 1,
                source_name: "b.json".to_string(),
                origin_index: 1,
            },
        ];

        let computed = compute_conflicts(&messages);
        assert_eq!(computed.total_messages, 2);
        assert_eq!(computed.conflicts.len(), 1);
        assert!(computed.conflicts[0].content1 != computed.conflicts[0].content2);
    }
}
