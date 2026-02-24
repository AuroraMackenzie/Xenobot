//! NLP API module for Xenobot HTTP API.
//!
//! Provides HTTP endpoints equivalent to Xenobot's `nlpApi` IPC methods.

use axum::{
    routing::{get, post},
    Json, Router,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sqlx::QueryBuilder;
use std::collections::{HashMap, HashSet};
use tracing::instrument;

use crate::ApiError;

/// NLP API router.
pub fn router() -> Router {
    Router::new()
        .route("/word-frequency", post(get_word_frequency))
        .route("/segment-text", post(segment_text))
        .route("/pos-tags", get(get_pos_tags))
}

// ==================== Request/Response Types ====================

/// Word frequency calculation parameters
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WordFrequencyParams {
    /// Session ID
    pub session_id: String,
    /// Language locale ('zh-CN' or 'en-US')
    pub locale: String,
    /// Time filter
    pub time_filter: Option<TimeFilter>,
    /// Member filter
    pub member_id: Option<i64>,
    /// Number of top words to return (default: 100)
    pub top_n: Option<u32>,
    /// Minimum word length (default: 2 for Chinese, 3 for English)
    pub min_word_length: Option<u32>,
    /// Minimum word count (default: 2)
    pub min_count: Option<u32>,
    /// POS tag filter mode: 'all', 'meaningful', or 'custom'
    pub pos_filter_mode: Option<String>,
    /// Custom POS tags for custom filter mode
    pub custom_pos_tags: Option<Vec<String>>,
    /// Whether to enable stopword filtering (default: true)
    pub enable_stopwords: Option<bool>,
}

/// Time filter for message queries
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeFilter {
    /// Start timestamp
    pub start_ts: Option<i64>,
    /// End timestamp
    pub end_ts: Option<i64>,
}

/// Word frequency result
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WordFrequencyResult {
    /// Word frequency items
    pub words: Vec<WordFrequencyItem>,
    /// Total words processed
    pub total_words: u64,
    /// Total messages processed
    pub total_messages: u64,
    /// Unique word count
    pub unique_words: u64,
    /// POS tag statistics (only for Chinese)
    pub pos_tag_stats: Option<Vec<PosTagStat>>,
}

/// Word frequency item
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WordFrequencyItem {
    /// Word text
    pub word: String,
    /// Occurrence count
    pub count: u64,
    /// Percentage of total words
    pub percentage: f64,
}

/// POS tag statistic
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosTagStat {
    /// POS tag identifier
    pub tag: String,
    /// Word count
    pub count: u64,
    /// Percentage of total words
    pub percentage: f64,
}

/// POS tag information
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosTagInfo {
    /// POS tag identifier (e.g., 'n', 'v', 'a')
    pub id: String,
    /// Display name in Chinese
    pub name_cn: String,
    /// Display name in English
    pub name_en: String,
    /// Description
    pub description: String,
    /// Whether considered meaningful content (for filtering)
    pub meaningful: bool,
}

// ==================== Handler Implementations ====================

#[derive(Debug, Deserialize)]
struct SegmentTextRequest {
    text: String,
    locale: String,
    min_length: Option<u32>,
}

fn is_chinese_locale(locale: &str) -> bool {
    locale.to_ascii_lowercase().starts_with("zh")
}

fn stopwords_zh() -> HashSet<&'static str> {
    [
        "的", "了", "和", "是", "在", "我", "你", "他", "她", "它", "这", "那", "有", "就", "也",
        "都", "而", "及", "与", "着", "啊", "吗", "呢", "吧",
    ]
    .into_iter()
    .collect()
}

fn stopwords_en() -> HashSet<&'static str> {
    [
        "the", "a", "an", "and", "or", "to", "of", "in", "on", "for", "at", "is", "are", "was",
        "were", "be", "been", "being", "that", "this", "it", "as", "by", "with",
    ]
    .into_iter()
    .collect()
}

fn tokenize_text(
    text: &str,
    locale: &str,
    min_length: usize,
    enable_stopwords: bool,
) -> Vec<String> {
    let is_zh = is_chinese_locale(locale);
    let re = Regex::new(r"[\p{Han}]|[\p{L}\p{N}_]+").expect("token regex");
    let stop_zh = stopwords_zh();
    let stop_en = stopwords_en();

    re.find_iter(text)
        .filter_map(|m| {
            let mut token = m.as_str().trim().to_string();
            if token.is_empty() {
                return None;
            }
            if !is_zh {
                token = token.to_ascii_lowercase();
            }
            if token.chars().count() < min_length {
                return None;
            }
            if enable_stopwords {
                if is_zh && stop_zh.contains(token.as_str()) {
                    return None;
                }
                if !is_zh && stop_en.contains(token.as_str()) {
                    return None;
                }
            }
            Some(token)
        })
        .collect()
}

#[instrument]
async fn segment_text(Json(req): Json<SegmentTextRequest>) -> Result<Json<Vec<String>>, ApiError> {
    let min_len =
        req.min_length
            .unwrap_or_else(|| if is_chinese_locale(&req.locale) { 1 } else { 2 }) as usize;
    let result = tokenize_text(&req.text, &req.locale, min_len, true);
    Ok(Json(result))
}

#[instrument]
async fn get_pos_tags() -> Result<Json<Vec<PosTagInfo>>, ApiError> {
    Ok(Json(vec![
        PosTagInfo {
            id: "n".to_string(),
            name_cn: "名词".to_string(),
            name_en: "Noun".to_string(),
            description: "实体、事物、概念".to_string(),
            meaningful: true,
        },
        PosTagInfo {
            id: "v".to_string(),
            name_cn: "动词".to_string(),
            name_en: "Verb".to_string(),
            description: "动作、行为、状态变化".to_string(),
            meaningful: true,
        },
        PosTagInfo {
            id: "a".to_string(),
            name_cn: "形容词".to_string(),
            name_en: "Adjective".to_string(),
            description: "性质、特征、程度".to_string(),
            meaningful: true,
        },
        PosTagInfo {
            id: "d".to_string(),
            name_cn: "副词".to_string(),
            name_en: "Adverb".to_string(),
            description: "修饰动词或形容词".to_string(),
            meaningful: true,
        },
        PosTagInfo {
            id: "x".to_string(),
            name_cn: "其他".to_string(),
            name_en: "Other".to_string(),
            description: "未归类词项".to_string(),
            meaningful: false,
        },
    ]))
}

#[instrument]
async fn get_word_frequency(
    Json(req): Json<WordFrequencyParams>,
) -> Result<Json<WordFrequencyResult>, ApiError> {
    let meta_id = req
        .session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("invalid session_id".to_string()))?;

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let mut qb: QueryBuilder<sqlx::Sqlite> =
        QueryBuilder::new("SELECT content FROM message WHERE meta_id = ");
    qb.push_bind(meta_id);
    if let Some(filter) = &req.time_filter {
        if let Some(start_ts) = filter.start_ts {
            qb.push(" AND ts >= ").push_bind(start_ts);
        }
        if let Some(end_ts) = filter.end_ts {
            qb.push(" AND ts <= ").push_bind(end_ts);
        }
    }
    if let Some(member_id) = req.member_id {
        qb.push(" AND sender_id = ").push_bind(member_id);
    }
    qb.push(" AND content IS NOT NULL AND TRIM(content) != ''");

    let rows: Vec<Option<String>> = qb
        .build_query_scalar()
        .fetch_all(pool.as_ref())
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let min_word_length =
        req.min_word_length
            .unwrap_or_else(|| if is_chinese_locale(&req.locale) { 1 } else { 2 }) as usize;
    let min_count = req.min_count.unwrap_or(2) as u64;
    let top_n = req.top_n.unwrap_or(100) as usize;
    let enable_stopwords = req.enable_stopwords.unwrap_or(true);

    let mut counts: HashMap<String, u64> = HashMap::new();
    let mut total_words = 0_u64;
    let mut total_messages = 0_u64;

    for content in rows.into_iter().flatten() {
        total_messages += 1;
        let tokens = tokenize_text(&content, &req.locale, min_word_length, enable_stopwords);
        total_words += tokens.len() as u64;
        for token in tokens {
            *counts.entry(token).or_insert(0) += 1;
        }
    }

    let mut sorted: Vec<(String, u64)> = counts
        .into_iter()
        .filter(|(_, count)| *count >= min_count)
        .collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    sorted.truncate(top_n);

    let words = sorted
        .into_iter()
        .map(|(word, count)| WordFrequencyItem {
            word,
            count,
            percentage: if total_words == 0 {
                0.0
            } else {
                (count as f64 / total_words as f64) * 100.0
            },
        })
        .collect::<Vec<_>>();

    let unique_words = words.len() as u64;
    let pos_tag_stats = if is_chinese_locale(&req.locale) {
        Some(vec![PosTagStat {
            tag: "x".to_string(),
            count: total_words,
            percentage: if total_words == 0 { 0.0 } else { 100.0 },
        }])
    } else {
        None
    };

    Ok(Json(WordFrequencyResult {
        words,
        total_words,
        total_messages,
        unique_words,
        pos_tag_stats,
    }))
}
