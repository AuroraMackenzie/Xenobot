// 时间过滤参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeFilter {
    pub start_ts: Option<i64>,
    pub end_ts: Option<i64>,
    pub member_id: Option<i64>,
}

// 成员活跃度查询结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberActivity {
    pub member_id: i64,
    pub platform_id: String,
    pub name: String,
    pub avatar: Option<String>,
    pub message_count: i64,
    pub percentage: f64,
}

// 时段分布查询结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeActivity {
    pub period: i64,
    pub message_count: i64,
}

// 消息长度分布结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageLengthDistribution {
    pub length_range: String,
    pub count: i64,
}

// 消息类型分布结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageTypeDistribution {
    pub msg_type: i64,
    pub count: i64,
}

// 时间范围结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub earliest: Option<i64>,
    pub latest: Option<i64>,
}

use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePool, FromRow, Result as SqlxResult};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ChatMeta {
    pub id: i64,
    pub name: String,
    pub platform: String,
    pub chat_type: String,
    pub imported_at: i64,
    pub group_id: Option<String>,
    pub group_avatar: Option<String>,
    pub owner_id: Option<String>,
    pub schema_version: i64,
    pub session_gap_threshold: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Member {
    pub id: i64,
    pub platform_id: String,
    pub account_name: Option<String>,
    pub group_nickname: Option<String>,
    pub aliases: Option<String>,
    pub avatar: Option<String>,
    pub roles: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Message {
    pub id: i64,
    pub sender_id: i64,
    pub sender_account_name: Option<String>,
    pub sender_group_nickname: Option<String>,
    pub ts: i64,
    pub msg_type: i64,
    pub content: Option<String>,
    pub reply_to_message_id: Option<String>,
    pub platform_message_id: Option<String>,
    pub meta_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ChatSession {
    pub id: i64,
    pub meta_id: i64,
    pub start_ts: i64,
    pub end_ts: i64,
    pub message_count: Option<i64>,
    pub is_manual: Option<bool>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MemberNameHistory {
    pub id: i64,
    pub member_id: i64,
    pub name_type: String,
    pub name: String,
    pub start_ts: i64,
    pub end_ts: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MessageContext {
    pub message_id: i64,
    pub session_id: i64,
    pub topic_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EmbeddingCache {
    pub id: i64,
    pub message_id: i64,
    pub content: String,
    pub embedding: Vec<u8>,
    pub model: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AnalysisCache {
    pub id: i64,
    pub meta_id: i64,
    pub analysis_type: String,
    pub result: String,
    pub params: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Sessions {
    pub id: i64,
    pub meta_id: i64,
    pub title: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SessionMessages {
    pub id: i64,
    pub session_id: i64,
    pub message_id: i64,
    pub order_index: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ImportProgress {
    pub id: i64,
    pub file_path: String,
    pub total_messages: Option<i32>,
    pub processed_messages: Option<i32>,
    pub status: Option<String>,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ImportSourceCheckpoint {
    pub id: i64,
    pub source_kind: String,
    pub source_path: String,
    pub fingerprint: String,
    pub file_size: i64,
    pub modified_at: i64,
    pub platform: Option<String>,
    pub chat_name: Option<String>,
    pub meta_id: Option<i64>,
    pub last_processed_at: i64,
    pub last_inserted_messages: i64,
    pub last_duplicate_messages: i64,
    pub status: String,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Conversations {
    pub id: i64,
    pub session_id: String,
    pub messages: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberStats {
    pub member_id: i64,
    pub account_name: Option<String>,
    pub message_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeDistribution {
    pub period: i64,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageLengthDistributionDetail {
    pub len: i64,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageLengthDistributionGrouped {
    pub range: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageLengthDistributionResult {
    pub detail: Vec<MessageLengthDistributionDetail>,
    pub grouped: Vec<MessageLengthDistributionGrouped>,
}

// ==================== Advanced Analysis Types ====================

// Catchphrase analysis types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatchphraseItem {
    pub content: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberCatchphrase {
    pub member_id: i64,
    pub platform_id: String,
    pub name: String,
    pub catchphrases: Vec<CatchphraseItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatchphraseAnalysis {
    pub members: Vec<MemberCatchphrase>,
}

// Mention analysis types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MentionMemberDetail {
    pub member_id: i64,
    pub platform_id: String,
    pub name: String,
    pub mentioned_count: i64,
    pub mentioner_count: i64,
    pub mention_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneWayMention {
    pub from_member_id: i64,
    pub from_name: String,
    pub to_member_id: i64,
    pub to_name: String,
    pub count: i64,
    pub ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwoWayMention {
    pub member_a_id: i64,
    pub member_a_name: String,
    pub member_b_id: i64,
    pub member_b_name: String,
    pub count_ab: i64,
    pub count_ba: i64,
    pub total: i64,
    pub ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MentionAnalysis {
    pub top_mentioners: Vec<MentionMemberDetail>,
    pub top_mentioned: Vec<MentionMemberDetail>,
    pub one_way: Vec<OneWayMention>,
    pub two_way: Vec<TwoWayMention>,
    pub total_mentions: i64,
    pub member_details: Vec<MentionMemberDetail>,
}

// Graph types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: i64,
    pub name: String,
    pub value: i64,
    pub symbol_size: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphLink {
    pub source: String,
    pub target: String,
    pub value: i64,
    pub raw_score: Option<f64>,
    pub expected_score: Option<f64>,
    pub co_occurrence_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MentionGraph {
    pub nodes: Vec<GraphNode>,
    pub links: Vec<GraphLink>,
    pub max_link_value: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterGraphOptions {
    pub look_ahead: i64,
    pub decay_seconds: f64,
    pub top_edges: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterGraphStats {
    pub total_members: i64,
    pub total_messages: i64,
    pub involved_members: i64,
    pub edge_count: i64,
    pub community_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterGraph {
    pub nodes: Vec<GraphNode>,
    pub links: Vec<GraphLink>,
    pub max_link_value: i64,
    pub communities: Vec<serde_json::Value>,
    pub stats: ClusterGraphStats,
}

// Laugh analysis types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaughRankItem {
    pub member_id: i64,
    pub platform_id: String,
    pub name: String,
    pub laugh_count: i64,
    pub message_count: i64,
    pub laugh_rate: f64,
    pub percentage: f64,
    pub keyword_distribution: Vec<(String, i64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaughTypeDistribution {
    pub keyword: String,
    pub count: i64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaughAnalysis {
    pub rank_by_rate: Vec<LaughRankItem>,
    pub rank_by_count: Vec<LaughRankItem>,
    pub type_distribution: Vec<LaughTypeDistribution>,
    pub total_laughs: i64,
    pub total_messages: i64,
    pub group_laugh_rate: f64,
}

pub struct Repository {
    pool: Arc<SqlitePool>,
}

impl Repository {
    pub fn new(pool: Arc<SqlitePool>) -> Self {
        Self { pool }
    }

    // Meta (ChatMeta) methods
    pub async fn create_chat(&self, meta: &ChatMeta) -> SqlxResult<i64> {
        let result = sqlx::query!(
            r#"
            INSERT INTO meta (name, platform, chat_type, imported_at, group_id, group_avatar, owner_id, schema_version, session_gap_threshold)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            meta.name,
            meta.platform,
            meta.chat_type,
            meta.imported_at,
            meta.group_id,
            meta.group_avatar,
            meta.owner_id,
            meta.schema_version,
            meta.session_gap_threshold
        )
        .execute(&*self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    pub async fn get_chat(&self, id: i64) -> SqlxResult<Option<ChatMeta>> {
        sqlx::query_as!(
            ChatMeta,
            r#"
            SELECT id as "id!: i64", name as "name!: String", platform as "platform!: String", chat_type as "chat_type!: String", imported_at as "imported_at!: i64", group_id as "group_id?", group_avatar as "group_avatar?", owner_id as "owner_id?", schema_version as "schema_version!: i64", session_gap_threshold as "session_gap_threshold!: i64"
            FROM meta WHERE id = ?1
            "#,
            id
        )
        .fetch_optional(&*self.pool)
        .await
    }

    pub async fn update_chat(&self, meta: &ChatMeta) -> SqlxResult<()> {
        sqlx::query!(
            r#"
            UPDATE meta SET name = ?2, platform = ?3, chat_type = ?4, imported_at = ?5, group_id = ?6,
            group_avatar = ?7, owner_id = ?8, schema_version = ?9, session_gap_threshold = ?10
            WHERE id = ?1
            "#,
            meta.id,
            meta.name,
            meta.platform,
            meta.chat_type,
            meta.imported_at,
            meta.group_id,
            meta.group_avatar,
            meta.owner_id,
            meta.schema_version,
            meta.session_gap_threshold
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_chats(
        &self,
        platform: Option<&str>,
        limit: i32,
        offset: i32,
    ) -> SqlxResult<Vec<ChatMeta>> {
        if let Some(p) = platform {
            sqlx::query_as!(
                ChatMeta,
                r#"
                SELECT id as "id!: i64", name as "name!: String", platform as "platform!: String", chat_type as "chat_type!: String", imported_at as "imported_at!: i64", group_id as "group_id?", group_avatar as "group_avatar?", owner_id as "owner_id?", schema_version as "schema_version!: i64", session_gap_threshold as "session_gap_threshold!: i64"
                FROM meta WHERE platform = ?1 ORDER BY imported_at DESC LIMIT ?2 OFFSET ?3
                "#,
                p,
                limit,
                offset
            )
            .fetch_all(&*self.pool)
            .await
        } else {
            sqlx::query_as!(
                ChatMeta,
                r#"
                SELECT id as "id!: i64", name as "name!: String", platform as "platform!: String", chat_type as "chat_type!: String", imported_at as "imported_at!: i64", group_id as "group_id?", group_avatar as "group_avatar?", owner_id as "owner_id?", schema_version as "schema_version!: i64", session_gap_threshold as "session_gap_threshold!: i64"
                FROM meta ORDER BY imported_at DESC LIMIT ?1 OFFSET ?2
                "#,
                limit,
                offset
            )
            .fetch_all(&*self.pool)
            .await
        }
    }

    pub async fn delete_chat(&self, id: i64) -> SqlxResult<()> {
        sqlx::query!("DELETE FROM meta WHERE id = ?1", id)
            .execute(&*self.pool)
            .await?;
        Ok(())
    }

    // Member methods
    pub async fn create_member(&self, member: &Member) -> SqlxResult<i64> {
        let result = sqlx::query!(
            r#"
            INSERT INTO member (platform_id, account_name, group_nickname, aliases, avatar, roles)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            member.platform_id,
            member.account_name,
            member.group_nickname,
            member.aliases,
            member.avatar,
            member.roles
        )
        .execute(&*self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    pub async fn get_member(&self, id: i64) -> SqlxResult<Option<Member>> {
        sqlx::query_as!(
            Member,
            r#"
            SELECT id, platform_id, account_name, group_nickname, aliases, avatar, roles
            FROM member WHERE id = ?1
            "#,
            id
        )
        .fetch_optional(&*self.pool)
        .await
    }

    pub async fn get_member_by_platform_id(&self, platform_id: &str) -> SqlxResult<Option<Member>> {
        sqlx::query_as::<_, Member>(
            r#"
            SELECT id, platform_id, account_name, group_nickname, aliases, avatar, roles
            FROM member WHERE platform_id = ?1
            "#,
        )
        .bind(platform_id)
        .fetch_optional(&*self.pool)
        .await
    }

    pub async fn update_member(&self, member: &Member) -> SqlxResult<()> {
        sqlx::query!(
            r#"
            UPDATE member SET platform_id = ?2, account_name = ?3, group_nickname = ?4,
            aliases = ?5, avatar = ?6, roles = ?7 WHERE id = ?1
            "#,
            member.id,
            member.platform_id,
            member.account_name,
            member.group_nickname,
            member.aliases,
            member.avatar,
            member.roles
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_member(&self, id: i64) -> SqlxResult<()> {
        sqlx::query!("DELETE FROM member WHERE id = ?1", id)
            .execute(&*self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_members(&self, limit: i32, offset: i32) -> SqlxResult<Vec<Member>> {
        sqlx::query_as!(
            Member,
            r#"
            SELECT id, platform_id, account_name, group_nickname, aliases, avatar, roles
            FROM member ORDER BY id LIMIT ?1 OFFSET ?2
            "#,
            limit,
            offset
        )
        .fetch_all(&*self.pool)
        .await
    }

    pub async fn get_or_create_member(
        &self,
        platform_id: &str,
        account_name: Option<&str>,
    ) -> SqlxResult<i64> {
        if let Some((existing_id,)) =
            sqlx::query_as::<_, (i64,)>("SELECT id FROM member WHERE platform_id = ?1")
                .bind(platform_id)
                .fetch_optional(&*self.pool)
                .await?
        {
            return Ok(existing_id);
        }

        let result = sqlx::query!(
            "INSERT INTO member (platform_id, account_name, aliases, roles) VALUES (?1, ?2, '[]', '[]')",
            platform_id,
            account_name
        )
        .execute(&*self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    // Message methods
    pub async fn create_message(&self, msg: &Message) -> SqlxResult<i64> {
        let result = sqlx::query!(
            r#"
            INSERT INTO message (sender_id, sender_account_name, sender_group_nickname, ts, msg_type, content, reply_to_message_id, platform_message_id, meta_id)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            msg.sender_id,
            msg.sender_account_name,
            msg.sender_group_nickname,
            msg.ts,
            msg.msg_type,
            msg.content,
            msg.reply_to_message_id,
            msg.platform_message_id,
            msg.meta_id
        )
        .execute(&*self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    pub async fn get_message(&self, id: i64) -> SqlxResult<Option<Message>> {
        sqlx::query_as!(
            Message,
            r#"
            SELECT id, sender_id, sender_account_name, sender_group_nickname, ts, msg_type, content, reply_to_message_id, platform_message_id, meta_id
            FROM message WHERE id = ?1
            "#,
            id
        )
        .fetch_optional(&*self.pool)
        .await
    }

    pub async fn update_message(&self, msg: &Message) -> SqlxResult<()> {
        sqlx::query!(
            r#"
            UPDATE message SET sender_id = ?2, sender_account_name = ?3, sender_group_nickname = ?4,
            ts = ?5, msg_type = ?6, content = ?7, reply_to_message_id = ?8, platform_message_id = ?9,
            meta_id = ?10 WHERE id = ?1
            "#,
            msg.id,
            msg.sender_id,
            msg.sender_account_name,
            msg.sender_group_nickname,
            msg.ts,
            msg.msg_type,
            msg.content,
            msg.reply_to_message_id,
            msg.platform_message_id,
            msg.meta_id
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_message(&self, id: i64) -> SqlxResult<()> {
        sqlx::query!("DELETE FROM message WHERE id = ?1", id)
            .execute(&*self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_messages(
        &self,
        meta_id: i64,
        limit: i32,
        offset: i32,
    ) -> SqlxResult<Vec<Message>> {
        sqlx::query_as::<_, Message>(
            r#"
            SELECT id, sender_id, sender_account_name, sender_group_nickname, ts, msg_type, content, reply_to_message_id, platform_message_id, meta_id
            FROM message WHERE meta_id = ?1 ORDER BY ts DESC LIMIT ?2 OFFSET ?3
            "#,
        )
        .bind(meta_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&*self.pool)
        .await
    }

    pub async fn search_messages(
        &self,
        meta_id: i64,
        keyword: &str,
        limit: i32,
    ) -> SqlxResult<Vec<Message>> {
        let pattern = format!("%{}%", keyword);
        sqlx::query_as::<_, Message>(
            r#"
            SELECT id, sender_id, sender_account_name, sender_group_nickname, ts, msg_type, content, reply_to_message_id, platform_message_id, meta_id
            FROM message WHERE meta_id = ?1 AND content LIKE ?2 ORDER BY ts DESC LIMIT ?3
            "#,
        )
        .bind(meta_id)
        .bind(pattern)
        .bind(limit)
        .fetch_all(&*self.pool)
        .await
    }

    pub async fn message_exists(
        &self,
        meta_id: i64,
        sender_id: i64,
        ts: i64,
        msg_type: i64,
        content: Option<&str>,
    ) -> SqlxResult<bool> {
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) as count
            FROM message
            WHERE meta_id = ?1
              AND sender_id = ?2
              AND ts = ?3
              AND msg_type = ?4
              AND COALESCE(content, '') = COALESCE(?5, '')
            "#,
        )
        .bind(meta_id)
        .bind(sender_id)
        .bind(ts)
        .bind(msg_type)
        .bind(content)
        .fetch_one(&*self.pool)
        .await?;
        Ok(count > 0)
    }

    pub async fn get_member_stats(&self, meta_id: i64) -> SqlxResult<Vec<MemberStats>> {
        #[derive(Debug, FromRow)]
        struct StatsRow {
            sender_id: i64,
            sender_account_name: Option<String>,
            msg_count: i64,
        }

        let rows: Vec<StatsRow> = sqlx::query_as!(
            StatsRow,
            r#"
            SELECT sender_id, sender_account_name, CAST(COUNT(*) AS INTEGER) as "msg_count!: i64"
            FROM message WHERE meta_id = ?1
            GROUP BY sender_id ORDER BY COUNT(*) DESC
            "#,
            meta_id
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| MemberStats {
                member_id: row.sender_id,
                account_name: row.sender_account_name,
                message_count: row.msg_count,
            })
            .collect())
    }

    pub async fn get_time_distribution(
        &self,
        meta_id: i64,
        group_by: &str,
    ) -> SqlxResult<Vec<TimeDistribution>> {
        let sql = match group_by {
            "hour" => "SELECT (ts / 3600) % 24 as period, COUNT(*) as count FROM message WHERE meta_id = ?1 GROUP BY period ORDER BY period",
            "weekday" => "SELECT (ts / 86400) % 7 as period, COUNT(*) as count FROM message WHERE meta_id = ?1 GROUP BY period ORDER BY period",
            "month" => "SELECT (ts / 2592000) % 12 as period, COUNT(*) as count FROM message WHERE meta_id = ?1 GROUP BY period ORDER BY period",
            _ => "SELECT ts / 86400 as period, COUNT(*) as count FROM message WHERE meta_id = ?1 GROUP BY period ORDER BY period",
        };

        #[derive(Debug, FromRow)]
        struct DistRow {
            period: i64,
            count: i64,
        }

        let rows: Vec<DistRow> = sqlx::query_as(sql)
            .bind(meta_id)
            .fetch_all(&*self.pool)
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| TimeDistribution {
                period: row.period,
                count: row.count,
            })
            .collect())
    }

    /// 构建时间过滤 WHERE 子句
    fn build_time_filter(
        &self,
        filter: &TimeFilter,
        table_alias: Option<&str>,
    ) -> (String, Vec<i64>) {
        let mut conditions = Vec::new();
        let mut params = Vec::new();

        let ts_column = table_alias
            .map(|a| format!("{}.ts", a))
            .unwrap_or_else(|| "ts".to_string());
        let sender_id_column = table_alias
            .map(|a| format!("{}.sender_id", a))
            .unwrap_or_else(|| "sender_id".to_string());

        if let Some(start_ts) = filter.start_ts {
            conditions.push(format!("{} >= ?", ts_column));
            params.push(start_ts);
        }
        if let Some(end_ts) = filter.end_ts {
            conditions.push(format!("{} <= ?", ts_column));
            params.push(end_ts);
        }
        if let Some(member_id) = filter.member_id {
            conditions.push(format!("{} = ?", sender_id_column));
            params.push(member_id);
        }

        let clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conditions.join(" AND "))
        };

        (clause, params)
    }

    fn bind_i64_params<'q, T>(
        mut query: sqlx::query::QueryAs<'q, sqlx::Sqlite, T, sqlx::sqlite::SqliteArguments<'q>>,
        params: &[i64],
    ) -> sqlx::query::QueryAs<'q, sqlx::Sqlite, T, sqlx::sqlite::SqliteArguments<'q>>
    where
        for<'r> T: sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> + Send + Unpin,
    {
        for param in params {
            query = query.bind(*param);
        }
        query
    }

    /// 构建排除系统消息的过滤条件
    fn build_system_message_filter(&self, existing_clause: &str) -> String {
        let system_filter = "COALESCE(m.account_name, '') != '系统消息'".to_string();
        if existing_clause.contains("WHERE") {
            format!("{} AND {}", existing_clause, system_filter)
        } else {
            format!(" WHERE {}", system_filter)
        }
    }

    /// 获取成员活跃度排行（带时间过滤）
    pub async fn get_member_activity_with_filter(
        &self,
        meta_id: i64,
        filter: Option<TimeFilter>,
    ) -> SqlxResult<Vec<MemberActivity>> {
        // 构建时间过滤条件
        let (time_clause, mut params) = if let Some(ref f) = filter {
            self.build_time_filter(f, Some("msg"))
        } else {
            (String::new(), Vec::new())
        };

        // 添加 meta_id 条件
        let meta_condition = " WHERE msg.meta_id = ?".to_string();
        let mut clause = meta_condition;
        if !time_clause.is_empty() {
            clause.push_str(" AND ");
            clause.push_str(&time_clause.trim_start_matches(" WHERE "));
        }
        params.insert(0, meta_id);

        // 构建排除系统消息的过滤条件
        let clause_with_system = self.build_system_message_filter(&clause);

        // 计算总消息数（排除系统消息）
        let total_query = format!(
            "SELECT COUNT(*) as count FROM message msg JOIN member m ON msg.sender_id = m.id {}",
            clause_with_system
        );
        #[derive(Debug, sqlx::FromRow)]
        struct TotalRow {
            count: i64,
        }
        let total_row: TotalRow = Self::bind_i64_params(sqlx::query_as(&total_query), &params)
            .fetch_one(&*self.pool)
            .await?;
        let total_messages = total_row.count;

        // 查询成员活跃度
        let member_query = format!(
            r#"
            SELECT
                m.id as member_id,
                m.platform_id as platform_id,
                COALESCE(m.group_nickname, m.account_name, m.platform_id) as name,
                m.avatar as avatar,
                COUNT(msg.id) as message_count
            FROM member m
            LEFT JOIN message msg ON m.id = msg.sender_id AND {}
            WHERE COALESCE(m.account_name, '') != '系统消息'
            GROUP BY m.id
            HAVING message_count > 0
            ORDER BY message_count DESC
            "#,
            clause_with_system.trim_start_matches(" WHERE ")
        );
        #[derive(Debug, sqlx::FromRow)]
        struct MemberRow {
            member_id: i64,
            platform_id: String,
            name: String,
            avatar: Option<String>,
            message_count: i64,
        }
        let rows: Vec<MemberRow> = Self::bind_i64_params(sqlx::query_as(&member_query), &params)
            .fetch_all(&*self.pool)
            .await?;

        // 计算百分比
        let result = rows
            .into_iter()
            .map(|row| {
                let percentage = if total_messages > 0 {
                    (row.message_count as f64 / total_messages as f64) * 100.0
                } else {
                    0.0
                };
                MemberActivity {
                    member_id: row.member_id,
                    platform_id: row.platform_id,
                    name: row.name,
                    avatar: row.avatar,
                    message_count: row.message_count,
                    percentage,
                }
            })
            .collect();

        Ok(result)
    }

    // ChatSession methods
    pub async fn create_chat_session(&self, session: &ChatSession) -> SqlxResult<i64> {
        let result = sqlx::query!(
            r#"
            INSERT INTO chat_session (meta_id, start_ts, end_ts, message_count, is_manual, summary)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            session.meta_id,
            session.start_ts,
            session.end_ts,
            session.message_count,
            session.is_manual,
            session.summary
        )
        .execute(&*self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    pub async fn get_chat_session(&self, id: i64) -> SqlxResult<Option<ChatSession>> {
        sqlx::query_as::<_, ChatSession>(
            r#"
            SELECT id, meta_id, start_ts, end_ts, message_count, is_manual, summary FROM chat_session WHERE id = ?1
            "#,
        )
        .bind(id)
        .fetch_optional(&*self.pool)
        .await
    }

    pub async fn update_chat_session(&self, session: &ChatSession) -> SqlxResult<()> {
        sqlx::query!(
            r#"
            UPDATE chat_session SET meta_id = ?2, start_ts = ?3, end_ts = ?4, message_count = ?5, is_manual = ?6, summary = ?7
            WHERE id = ?1
            "#,
            session.id,
            session.meta_id,
            session.start_ts,
            session.end_ts,
            session.message_count,
            session.is_manual,
            session.summary
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_chat_session(&self, id: i64) -> SqlxResult<()> {
        sqlx::query!("DELETE FROM chat_session WHERE id = ?1", id)
            .execute(&*self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_sessions(
        &self,
        meta_id: i64,
        limit: i32,
        offset: i32,
    ) -> SqlxResult<Vec<ChatSession>> {
        sqlx::query_as::<_, ChatSession>(
            r#"
            SELECT id, meta_id, start_ts, end_ts, message_count, is_manual, summary
            FROM chat_session WHERE meta_id = ?1 ORDER BY start_ts DESC LIMIT ?2 OFFSET ?3
            "#,
        )
        .bind(meta_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&*self.pool)
        .await
    }

    // MemberNameHistory methods
    pub async fn create_member_name_history(&self, history: &MemberNameHistory) -> SqlxResult<i64> {
        let result = sqlx::query!(
            r#"
            INSERT INTO member_name_history (member_id, name_type, name, start_ts, end_ts)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            history.member_id,
            history.name_type,
            history.name,
            history.start_ts,
            history.end_ts
        )
        .execute(&*self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    pub async fn get_member_name_history_by_id(
        &self,
        id: i64,
    ) -> SqlxResult<Option<MemberNameHistory>> {
        sqlx::query_as!(
            MemberNameHistory,
            r#"
            SELECT id, member_id, name_type, name, start_ts, end_ts
            FROM member_name_history WHERE id = ?1
            "#,
            id
        )
        .fetch_optional(&*self.pool)
        .await
    }

    pub async fn get_member_name_history_by_member_id(
        &self,
        member_id: i64,
    ) -> SqlxResult<Vec<MemberNameHistory>> {
        sqlx::query_as::<_, MemberNameHistory>(
            r#"
            SELECT id, member_id, name_type, name, start_ts, end_ts
            FROM member_name_history WHERE member_id = ?1 ORDER BY start_ts DESC
            "#,
        )
        .bind(member_id)
        .fetch_all(&*self.pool)
        .await
    }

    pub async fn update_member_name_history_end_ts(&self, id: i64, end_ts: i64) -> SqlxResult<()> {
        sqlx::query!(
            r#"
            UPDATE member_name_history SET end_ts = ?1 WHERE id = ?2
            "#,
            end_ts,
            id
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_member_name_history(&self, id: i64) -> SqlxResult<()> {
        sqlx::query!("DELETE FROM member_name_history WHERE id = ?1", id)
            .execute(&*self.pool)
            .await?;
        Ok(())
    }

    // MessageContext methods
    pub async fn create_message_context(&self, context: &MessageContext) -> SqlxResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO message_context (message_id, session_id, topic_id)
            VALUES (?1, ?2, ?3)
            "#,
            context.message_id,
            context.session_id,
            context.topic_id
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_message_context_by_message_id(
        &self,
        message_id: i64,
    ) -> SqlxResult<Option<MessageContext>> {
        sqlx::query_as!(
            MessageContext,
            r#"
            SELECT message_id, session_id, topic_id
            FROM message_context WHERE message_id = ?1
            "#,
            message_id
        )
        .fetch_optional(&*self.pool)
        .await
    }

    pub async fn get_message_contexts_by_session_id(
        &self,
        session_id: i64,
    ) -> SqlxResult<Vec<MessageContext>> {
        sqlx::query_as::<_, MessageContext>(
            r#"
            SELECT message_id, session_id, topic_id
            FROM message_context WHERE session_id = ?1 ORDER BY message_id
            "#,
        )
        .bind(session_id)
        .fetch_all(&*self.pool)
        .await
    }

    pub async fn delete_message_context_by_message_id(&self, message_id: i64) -> SqlxResult<()> {
        sqlx::query!(
            "DELETE FROM message_context WHERE message_id = ?1",
            message_id
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    // EmbeddingCache methods
    pub async fn create_embedding_cache(&self, cache: &EmbeddingCache) -> SqlxResult<i64> {
        let result = sqlx::query!(
            r#"
            INSERT INTO embedding_cache (message_id, content, embedding, model, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            cache.message_id,
            cache.content,
            cache.embedding,
            cache.model,
            cache.created_at
        )
        .execute(&*self.pool)
        .await?;
        Ok(result.last_insert_rowid())
    }

    pub async fn get_embedding_cache_by_id(&self, id: i64) -> SqlxResult<Option<EmbeddingCache>> {
        sqlx::query_as!(
            EmbeddingCache,
            r#"
            SELECT id, message_id, content, embedding, model, created_at
            FROM embedding_cache WHERE id = ?1
            "#,
            id
        )
        .fetch_optional(&*self.pool)
        .await
    }

    pub async fn get_embedding_cache_by_message_id(
        &self,
        message_id: i64,
    ) -> SqlxResult<Vec<EmbeddingCache>> {
        sqlx::query_as::<_, EmbeddingCache>(
            r#"
            SELECT id, message_id, content, embedding, model, created_at
            FROM embedding_cache WHERE message_id = ?1 ORDER BY created_at DESC
            "#,
        )
        .bind(message_id)
        .fetch_all(&*self.pool)
        .await
    }

    pub async fn get_embedding_cache_by_model(
        &self,
        model: &str,
    ) -> SqlxResult<Vec<EmbeddingCache>> {
        sqlx::query_as!(
            EmbeddingCache,
            r#"
            SELECT id, message_id, content, embedding, model, created_at
            FROM embedding_cache WHERE model = ?1 ORDER BY created_at DESC
            "#,
            model
        )
        .fetch_all(&*self.pool)
        .await
    }

    pub async fn delete_embedding_cache(&self, id: i64) -> SqlxResult<()> {
        sqlx::query!("DELETE FROM embedding_cache WHERE id = ?1", id)
            .execute(&*self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_embedding_cache_by_message_id(&self, message_id: i64) -> SqlxResult<()> {
        sqlx::query!(
            "DELETE FROM embedding_cache WHERE message_id = ?1",
            message_id
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    // AnalysisCache methods
    pub async fn create_analysis_cache(&self, cache: &AnalysisCache) -> SqlxResult<i64> {
        let result = sqlx::query!(
            r#"
            INSERT INTO analysis_cache (meta_id, analysis_type, result, params, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            cache.meta_id,
            cache.analysis_type,
            cache.result,
            cache.params,
            cache.created_at
        )
        .execute(&*self.pool)
        .await?;
        Ok(result.last_insert_rowid())
    }

    pub async fn get_analysis_cache_by_id(&self, id: i64) -> SqlxResult<Option<AnalysisCache>> {
        sqlx::query_as!(
            AnalysisCache,
            r#"
            SELECT id, meta_id, analysis_type, result, params, created_at
            FROM analysis_cache WHERE id = ?1
            "#,
            id
        )
        .fetch_optional(&*self.pool)
        .await
    }

    pub async fn get_analysis_cache_by_meta_id(
        &self,
        meta_id: i64,
    ) -> SqlxResult<Vec<AnalysisCache>> {
        sqlx::query_as::<_, AnalysisCache>(
            r#"
            SELECT id, meta_id, analysis_type, result, params, created_at
            FROM analysis_cache WHERE meta_id = ?1 ORDER BY created_at DESC
            "#,
        )
        .bind(meta_id)
        .fetch_all(&*self.pool)
        .await
    }

    pub async fn get_analysis_cache_by_type(
        &self,
        meta_id: i64,
        analysis_type: &str,
    ) -> SqlxResult<Option<AnalysisCache>> {
        sqlx::query_as::<_, AnalysisCache>(
            r#"
            SELECT id, meta_id, analysis_type, result, params, created_at
            FROM analysis_cache WHERE meta_id = ?1 AND analysis_type = ?2
            "#,
        )
        .bind(meta_id)
        .bind(analysis_type)
        .fetch_optional(&*self.pool)
        .await
    }

    pub async fn delete_analysis_cache(&self, id: i64) -> SqlxResult<()> {
        sqlx::query!("DELETE FROM analysis_cache WHERE id = ?1", id)
            .execute(&*self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_analysis_cache_by_meta_id(&self, meta_id: i64) -> SqlxResult<()> {
        sqlx::query!("DELETE FROM analysis_cache WHERE meta_id = ?1", meta_id)
            .execute(&*self.pool)
            .await?;
        Ok(())
    }

    // Advanced Analysis methods

    /// 获取口头禅分析
    pub async fn get_catchphrase_analysis(
        &self,
        meta_id: i64,
        filter: Option<TimeFilter>,
    ) -> SqlxResult<CatchphraseAnalysis> {
        // 构建时间过滤条件
        let (time_clause, mut params) = if let Some(ref f) = filter {
            self.build_time_filter(f, Some("msg"))
        } else {
            (String::new(), Vec::new())
        };

        // 添加 meta_id 条件
        let meta_condition = " WHERE msg.meta_id = ?".to_string();
        let mut clause = meta_condition;
        if !time_clause.is_empty() {
            clause.push_str(" AND ");
            clause.push_str(&time_clause.trim_start_matches(" WHERE "));
        }
        params.insert(0, meta_id);

        // 构建排除系统消息的过滤条件
        let clause_with_system = self.build_system_message_filter(&clause);

        // 查询重复短语
        let query = format!(
            r#"
            SELECT
              m.id as member_id,
              m.platform_id as platform_id,
              COALESCE(m.group_nickname, m.account_name, m.platform_id) as name,
              TRIM(msg.content) as content,
              COUNT(*) as count
            FROM message msg
            JOIN member m ON msg.sender_id = m.id
            {}
              AND msg.msg_type = 0
              AND msg.content IS NOT NULL
              AND LENGTH(TRIM(msg.content)) >= 2
            GROUP BY m.id, TRIM(msg.content)
            ORDER BY m.id, count DESC
            "#,
            clause_with_system
        );

        #[derive(Debug, sqlx::FromRow)]
        struct CatchphraseRow {
            member_id: i64,
            platform_id: String,
            name: String,
            content: String,
            count: i64,
        }

        let rows: Vec<CatchphraseRow> = Self::bind_i64_params(sqlx::query_as(&query), &params)
            .fetch_all(&*self.pool)
            .await?;

        // 按成员分组，每个成员保留前5个口头禅
        let mut members_map: std::collections::HashMap<i64, MemberCatchphrase> =
            std::collections::HashMap::new();
        for row in rows {
            let member_entry =
                members_map
                    .entry(row.member_id)
                    .or_insert_with(|| MemberCatchphrase {
                        member_id: row.member_id,
                        platform_id: row.platform_id.clone(),
                        name: row.name.clone(),
                        catchphrases: Vec::new(),
                    });

            // 每个成员最多保留5个口头禅
            if member_entry.catchphrases.len() < 5 {
                member_entry.catchphrases.push(CatchphraseItem {
                    content: row.content,
                    count: row.count,
                });
            }
        }

        // 转换为向量并按总catchphrase数排序
        let mut members: Vec<MemberCatchphrase> = members_map.into_values().collect();
        members.sort_by(|a, b| {
            let a_total: i64 = a.catchphrases.iter().map(|c| c.count).sum();
            let b_total: i64 = b.catchphrases.iter().map(|c| c.count).sum();
            b_total.cmp(&a_total)
        });

        Ok(CatchphraseAnalysis { members })
    }

    /// 获取提及分析
    pub async fn get_mention_analysis(
        &self,
        meta_id: i64,
        filter: Option<TimeFilter>,
    ) -> SqlxResult<MentionAnalysis> {
        use regex::Regex;
        use std::collections::{HashMap, HashSet};

        // 构建时间过滤条件
        let (time_clause, mut params) = if let Some(ref f) = filter {
            self.build_time_filter(f, Some("msg"))
        } else {
            (String::new(), Vec::new())
        };

        // 添加 meta_id 条件
        let meta_condition = " WHERE msg.meta_id = ?".to_string();
        let mut clause = meta_condition;
        if !time_clause.is_empty() {
            clause.push_str(" AND ");
            clause.push_str(&time_clause.trim_start_matches(" WHERE "));
        }
        params.insert(0, meta_id);

        // 构建排除系统消息的过滤条件
        let clause_with_system = self.build_system_message_filter(&clause);

        // 1. 查询所有成员及其历史名称
        let members_query = r#"
            SELECT m.id, m.platform_id, m.account_name, m.group_nickname, 
                   COALESCE((SELECT GROUP_CONCAT(name) FROM member_name_history WHERE member_id = m.id), "") as history_names
            FROM member m
            WHERE EXISTS (SELECT 1 FROM message msg WHERE msg.sender_id = m.id AND msg.meta_id = ?1)
            AND COALESCE(m.account_name, "") != "系统消息"
        "#;

        #[derive(Debug, sqlx::FromRow)]
        struct MemberRow {
            id: i64,
            platform_id: String,
            account_name: Option<String>,
            group_nickname: Option<String>,
            history_names: String,
        }

        let member_rows: Vec<MemberRow> = sqlx::query_as(members_query)
            .bind(meta_id)
            .fetch_all(&*self.pool)
            .await?;

        // 构建名称到成员ID的映射
        let mut name_to_id: HashMap<String, Vec<i64>> = HashMap::new();
        for row in &member_rows {
            let names = vec![
                row.platform_id.clone(),
                row.account_name.clone().unwrap_or_default(),
                row.group_nickname.clone().unwrap_or_default(),
            ];
            for name in names.into_iter().filter(|n| !n.is_empty()) {
                name_to_id
                    .entry(name.to_lowercase())
                    .or_insert_with(Vec::new)
                    .push(row.id);
            }
            // 历史名称（逗号分隔）
            if !row.history_names.is_empty() {
                for name in row.history_names.split(',') {
                    let trimmed = name.trim();
                    if !trimmed.is_empty() {
                        name_to_id
                            .entry(trimmed.to_lowercase())
                            .or_insert_with(Vec::new)
                            .push(row.id);
                    }
                }
            }
        }

        // 2. 查询包含@的消息
        let messages_query = format!(
            r#"
            SELECT msg.sender_id, msg.content
            FROM message msg
            JOIN member m ON msg.sender_id = m.id
            {}
              AND msg.msg_type = 0
              AND msg.content IS NOT NULL
              AND msg.content LIKE '%@%'
            "#,
            clause_with_system
        );

        #[derive(Debug, sqlx::FromRow)]
        struct MessageRow {
            sender_id: i64,
            content: String,
        }

        let message_rows: Vec<MessageRow> =
            Self::bind_i64_params(sqlx::query_as(&messages_query), &params)
                .fetch_all(&*self.pool)
                .await?;

        // 3. 解析提及并构建矩阵
        let mention_regex = Regex::new(r"@([^\s@]+)").unwrap();
        let mut mention_matrix: HashMap<i64, HashMap<i64, i64>> = HashMap::new();
        let mut mentioned_count: HashMap<i64, i64> = HashMap::new();
        let mut mentioner_count: HashMap<i64, i64> = HashMap::new();

        for msg in message_rows {
            let sender_id = msg.sender_id;
            let content = msg.content;

            // 收集本消息中提及的成员ID（去重）
            let mut mentioned_in_msg: HashSet<i64> = HashSet::new();
            for cap in mention_regex.captures_iter(&content) {
                let mention_name = cap.get(1).unwrap().as_str().to_lowercase();
                // 查找匹配的成员ID
                if let Some(ids) = name_to_id.get(&mention_name) {
                    // 如果有多个匹配，选择第一个
                    if let Some(&matched_id) = ids.first() {
                        // 排除自我提及
                        if matched_id != sender_id {
                            mentioned_in_msg.insert(matched_id);
                        }
                    }
                }
            }

            // 更新矩阵和计数
            for &mentioned_id in &mentioned_in_msg {
                *mention_matrix
                    .entry(sender_id)
                    .or_default()
                    .entry(mentioned_id)
                    .or_insert(0) += 1;
                *mentioned_count.entry(mentioned_id).or_insert(0) += 1;
            }
            if !mentioned_in_msg.is_empty() {
                *mentioner_count.entry(sender_id).or_insert(0) += 1;
            }
        }

        let total_mentions: i64 = mention_matrix
            .values()
            .map(|m| m.values().sum::<i64>())
            .sum();

        // 4. 构建成员详情
        let mut member_details: Vec<MentionMemberDetail> = Vec::new();
        for row in member_rows {
            let mentioned = mentioned_count.get(&row.id).copied().unwrap_or(0);
            let mentioner = mentioner_count.get(&row.id).copied().unwrap_or(0);
            let mention_rate = if mentioned + mentioner > 0 {
                (mentioned as f64) / (mentioned + mentioner) as f64 * 100.0
            } else {
                0.0
            };
            let platform_id = row.platform_id.clone();
            let name = row
                .group_nickname
                .clone()
                .or(row.account_name.clone())
                .unwrap_or_else(|| platform_id.clone());

            member_details.push(MentionMemberDetail {
                member_id: row.id,
                platform_id,
                name,
                mentioned_count: mentioned,
                mentioner_count: mentioner,
                mention_rate,
            });
        }

        // 排序：最常提及他人
        let mut top_mentioners = member_details.clone();
        top_mentioners.sort_by(|a, b| b.mentioner_count.cmp(&a.mentioner_count));
        let top_mentioners = top_mentioners.into_iter().take(10).collect();

        // 排序：最常被提及
        let mut top_mentioned = member_details.clone();
        top_mentioned.sort_by(|a, b| b.mentioned_count.cmp(&a.mentioned_count));
        let top_mentioned = top_mentioned.into_iter().take(10).collect();

        // 5. 计算单向关系 (A @ B 比例 >= 80%)
        let mut one_way: Vec<OneWayMention> = Vec::new();
        for (&from_id, targets) in &mention_matrix {
            for (&to_id, &count) in targets {
                let reverse_count = mention_matrix
                    .get(&to_id)
                    .and_then(|m| m.get(&from_id))
                    .copied()
                    .unwrap_or(0);
                let total = count + reverse_count;
                if total >= 3 {
                    let ratio = count as f64 / total as f64;
                    if ratio >= 0.8 {
                        let from_name = member_details
                            .iter()
                            .find(|m| m.member_id == from_id)
                            .map(|m| m.name.clone())
                            .unwrap_or_default();
                        let to_name = member_details
                            .iter()
                            .find(|m| m.member_id == to_id)
                            .map(|m| m.name.clone())
                            .unwrap_or_default();
                        one_way.push(OneWayMention {
                            from_member_id: from_id,
                            from_name,
                            to_member_id: to_id,
                            to_name,
                            count,
                            ratio,
                        });
                    }
                }
            }
        }
        one_way.sort_by(|a, b| b.count.cmp(&a.count));

        // 6. 计算双向关系 (CP)
        let mut two_way: Vec<TwoWayMention> = Vec::new();
        for (&a_id, targets) in &mention_matrix {
            for (&b_id, &ab_count) in targets {
                if a_id >= b_id {
                    continue;
                }
                let ba_count = mention_matrix
                    .get(&b_id)
                    .and_then(|m| m.get(&a_id))
                    .copied()
                    .unwrap_or(0);
                if ab_count > 0 && ba_count > 0 {
                    let total = ab_count + ba_count;
                    if total >= 5 {
                        let ratio = if ab_count >= ba_count {
                            ba_count as f64 / ab_count as f64
                        } else {
                            ab_count as f64 / ba_count as f64
                        };
                        if ratio >= 0.3 {
                            let a_name = member_details
                                .iter()
                                .find(|m| m.member_id == a_id)
                                .map(|m| m.name.clone())
                                .unwrap_or_default();
                            let b_name = member_details
                                .iter()
                                .find(|m| m.member_id == b_id)
                                .map(|m| m.name.clone())
                                .unwrap_or_default();
                            two_way.push(TwoWayMention {
                                member_a_id: a_id,
                                member_a_name: a_name,
                                member_b_id: b_id,
                                member_b_name: b_name,
                                count_ab: ab_count,
                                count_ba: ba_count,
                                total,
                                ratio,
                            });
                        }
                    }
                }
            }
        }
        two_way.sort_by(|a, b| b.total.cmp(&a.total));

        Ok(MentionAnalysis {
            top_mentioners,
            top_mentioned,
            one_way,
            two_way,
            total_mentions,
            member_details,
        })
    } // end mention graph
    pub async fn get_mention_graph(
        &self,
        meta_id: i64,
        filter: Option<TimeFilter>,
    ) -> SqlxResult<MentionGraph> {
        use regex::Regex;
        use std::collections::{HashMap, HashSet};

        // Build time filter
        let (time_clause, mut params) = if let Some(ref f) = filter {
            self.build_time_filter(f, Some("msg"))
        } else {
            (String::new(), Vec::new())
        };

        // Add meta_id condition
        let meta_condition = " WHERE msg.meta_id = ?".to_string();
        let mut clause = meta_condition;
        if !time_clause.is_empty() {
            clause.push_str(" AND ");
            clause.push_str(&time_clause.trim_start_matches(" WHERE "));
        }
        params.insert(0, meta_id);

        // Build exclude system messages filter
        let clause_with_system = self.build_system_message_filter(&clause);

        // 1. Query all members with message counts (including zero-message members)
        // We need to LEFT JOIN messages with the filter to count messages per member
        // Use the same pattern as get_member_activity_with_filter but remove HAVING message_count > 0
        let member_query = format!(
            r#"
            SELECT
                m.id as member_id,
                m.platform_id as platform_id,
                COALESCE(m.group_nickname, m.account_name, m.platform_id) as name,
                COUNT(msg.id) as message_count
            FROM member m
            LEFT JOIN message msg ON m.id = msg.sender_id AND {}
            WHERE COALESCE(m.account_name, '') != '系统消息'
            GROUP BY m.id
            "#,
            clause_with_system.trim_start_matches(" WHERE ")
        );
        #[derive(Debug, sqlx::FromRow)]
        struct MemberRow {
            member_id: i64,
            platform_id: String,
            name: String,
            message_count: i64,
        }
        let member_rows: Vec<MemberRow> =
            Self::bind_i64_params(sqlx::query_as(&member_query), &params)
                .fetch_all(&*self.pool)
                .await?;

        if member_rows.is_empty() {
            return Ok(MentionGraph {
                nodes: Vec::new(),
                links: Vec::new(),
                max_link_value: 0,
            });
        }

        // 2. Build name to member ID mapping (including historical nicknames)
        let mut name_to_member_id: HashMap<String, i64> = HashMap::new();
        let mut member_id_to_info: HashMap<i64, (String, i64)> = HashMap::new(); // (name, message_count)

        for row in &member_rows {
            let member_id = row.member_id;
            let name = row.name.clone();
            member_id_to_info.insert(member_id, (name.clone(), row.message_count));
            name_to_member_id.insert(name, member_id);

            // Query historical nicknames
            let histories = self.get_member_name_history_by_member_id(member_id).await?;
            for history in histories {
                if !name_to_member_id.contains_key(&history.name) {
                    name_to_member_id.insert(history.name, member_id);
                }
            }
        }

        // 3. Query messages containing '@' (with time filter, exclude system messages)
        // Build WHERE clause for messages containing '@'
        let mut where_clause = clause_with_system.clone();
        if where_clause.contains("WHERE") {
            where_clause.push_str(
                " AND msg.msg_type = 0 AND msg.content IS NOT NULL AND msg.content LIKE '%@%'",
            );
        } else {
            where_clause =
                " WHERE msg.msg_type = 0 AND msg.content IS NOT NULL AND msg.content LIKE '%@%'"
                    .to_string();
        }

        let message_query = format!(
            r#"
            SELECT msg.sender_id as sender_id, msg.content
            FROM message msg
            JOIN member m ON msg.sender_id = m.id
            {}
            "#,
            where_clause
        );
        #[derive(Debug, sqlx::FromRow)]
        struct MessageRow {
            sender_id: i64,
            content: String,
        }
        let message_rows: Vec<MessageRow> =
            Self::bind_i64_params(sqlx::query_as(&message_query), &params)
                .fetch_all(&*self.pool)
                .await?;

        // 4. Parse @mentions and build mention matrix
        let mention_regex = Regex::new(r"@([^\s@]+)").unwrap();
        let mut mention_matrix: HashMap<i64, HashMap<i64, i64>> = HashMap::new();

        for msg in &message_rows {
            let sender_id = msg.sender_id;
            let content = &msg.content;
            let mut mentioned_in_this_msg: HashSet<i64> = HashSet::new();

            for cap in mention_regex.captures_iter(content) {
                if let Some(mentioned_name) = cap.get(1) {
                    let name = mentioned_name.as_str().to_string();
                    if let Some(&mentioned_id) = name_to_member_id.get(&name) {
                        if mentioned_id != sender_id
                            && !mentioned_in_this_msg.contains(&mentioned_id)
                        {
                            mentioned_in_this_msg.insert(mentioned_id);
                            let from_map =
                                mention_matrix.entry(sender_id).or_insert_with(HashMap::new);
                            *from_map.entry(mentioned_id).or_insert(0) += 1;
                        }
                    }
                }
            }
        }

        // 5. Build nodes for involved members
        let involved_member_ids: HashSet<i64> = mention_matrix
            .keys()
            .chain(mention_matrix.values().flat_map(|m| m.keys()))
            .cloned()
            .collect();

        if involved_member_ids.is_empty() {
            return Ok(MentionGraph {
                nodes: Vec::new(),
                links: Vec::new(),
                max_link_value: 0,
            });
        }

        // Find max message count among involved members
        let max_message_count = involved_member_ids
            .iter()
            .filter_map(|&id| member_id_to_info.get(&id).map(|(_, count)| *count))
            .max()
            .unwrap_or(1);

        let mut nodes: Vec<GraphNode> = Vec::new();
        for &member_id in &involved_member_ids {
            if let Some((name, message_count)) = member_id_to_info.get(&member_id) {
                // Node size 20-60 based on message count proportion
                let symbol_size = 20.0 + (*message_count as f64 / max_message_count as f64) * 40.0;
                nodes.push(GraphNode {
                    id: member_id,
                    name: name.clone(),
                    value: *message_count,
                    symbol_size: symbol_size.round() as i64,
                });
            }
        }

        // 6. Build links (using member names for ECharts compatibility)
        let mut links: Vec<GraphLink> = Vec::new();
        let mut max_link_value = 0;

        for (from_id, to_map) in &mention_matrix {
            let from_info = member_id_to_info.get(from_id);
            if from_info.is_none() {
                continue;
            }
            let from_name = &from_info.unwrap().0;

            for (to_id, count) in to_map {
                let to_info = member_id_to_info.get(to_id);
                if to_info.is_none() {
                    continue;
                }
                let to_name = &to_info.unwrap().0;

                links.push(GraphLink {
                    source: from_name.clone(),
                    target: to_name.clone(),
                    value: *count,
                    raw_score: None,
                    expected_score: None,
                    co_occurrence_count: None,
                });

                if *count > max_link_value {
                    max_link_value = *count;
                }
            }
        }

        Ok(MentionGraph {
            // test
            nodes,
            links,
            max_link_value,
        })
    } // end mention graph

    // Sessions methods
    pub async fn create_session(&self, session: &Sessions) -> SqlxResult<i64> {
        let result = sqlx::query!(
            r#"
            INSERT INTO sessions (meta_id, title, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            session.meta_id,
            session.title,
            session.created_at,
            session.updated_at
        )
        .execute(&*self.pool)
        .await?;
        Ok(result.last_insert_rowid())
    }

    pub async fn get_session(&self, id: i64) -> SqlxResult<Option<Sessions>> {
        sqlx::query_as!(
            Sessions,
            r#"
            SELECT id, meta_id, title, created_at, updated_at
            FROM sessions WHERE id = ?1
            "#,
            id
        )
        .fetch_optional(&*self.pool)
        .await
    }

    pub async fn list_sessions_by_meta_id(&self, meta_id: i64) -> SqlxResult<Vec<Sessions>> {
        sqlx::query_as!(
            Sessions,
            r#"
            SELECT id, meta_id, title, created_at, updated_at
            FROM sessions WHERE meta_id = ?1 ORDER BY created_at DESC
            "#,
            meta_id
        )
        .fetch_all(&*self.pool)
        .await
    }

    pub async fn update_session(
        &self,
        id: i64,
        title: Option<&str>,
        updated_at: i64,
    ) -> SqlxResult<()> {
        sqlx::query!(
            r#"
            UPDATE sessions SET title = ?1, updated_at = ?2 WHERE id = ?3
            "#,
            title,
            updated_at,
            id
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_session(&self, id: i64) -> SqlxResult<()> {
        sqlx::query!("DELETE FROM sessions WHERE id = ?1", id)
            .execute(&*self.pool)
            .await?;
        Ok(())
    }

    // SessionMessages methods
    pub async fn create_session_message(
        &self,
        session_message: &SessionMessages,
    ) -> SqlxResult<i64> {
        let result = sqlx::query!(
            r#"
            INSERT INTO session_messages (session_id, message_id, order_index)
            VALUES (?1, ?2, ?3)
            "#,
            session_message.session_id,
            session_message.message_id,
            session_message.order_index
        )
        .execute(&*self.pool)
        .await?;
        Ok(result.last_insert_rowid())
    }

    pub async fn get_session_messages_by_session_id(
        &self,
        session_id: i64,
    ) -> SqlxResult<Vec<SessionMessages>> {
        sqlx::query_as::<_, SessionMessages>(
            r#"
            SELECT id, session_id, message_id, order_index
            FROM session_messages WHERE session_id = ?1 ORDER BY order_index
            "#,
        )
        .bind(session_id)
        .fetch_all(&*self.pool)
        .await
    }

    pub async fn update_order_index(&self, id: i64, order_index: i32) -> SqlxResult<()> {
        sqlx::query!(
            r#"
            UPDATE session_messages SET order_index = ?1 WHERE id = ?2
            "#,
            order_index,
            id
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_session_message(&self, id: i64) -> SqlxResult<()> {
        sqlx::query!("DELETE FROM session_messages WHERE id = ?1", id)
            .execute(&*self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_session_messages_by_session_id(&self, session_id: i64) -> SqlxResult<()> {
        sqlx::query!(
            "DELETE FROM session_messages WHERE session_id = ?1",
            session_id
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    // ImportProgress methods
    pub async fn create_import_progress(&self, progress: &ImportProgress) -> SqlxResult<i64> {
        let result = sqlx::query!(
            r#"
            INSERT INTO import_progress (file_path, total_messages, processed_messages, status, started_at, completed_at, error_message)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            progress.file_path,
            progress.total_messages,
            progress.processed_messages,
            progress.status,
            progress.started_at,
            progress.completed_at,
            progress.error_message
        )
        .execute(&*self.pool)
        .await?;
        Ok(result.last_insert_rowid())
    }

    pub async fn get_import_progress(&self, id: i64) -> SqlxResult<Option<ImportProgress>> {
        sqlx::query_as::<_, ImportProgress>(
            r#"
            SELECT id, file_path, total_messages, processed_messages, status, started_at, completed_at, error_message
            FROM import_progress WHERE id = ?1
            "#,
        )
        .bind(id)
        .fetch_optional(&*self.pool)
        .await
    }

    pub async fn update_progress(
        &self,
        id: i64,
        processed_messages: i32,
        status: &str,
    ) -> SqlxResult<()> {
        sqlx::query!(
            r#"
            UPDATE import_progress SET processed_messages = ?2, status = ?3 WHERE id = ?1
            "#,
            id,
            processed_messages,
            status
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn complete_import(&self, id: i64, completed_at: i64) -> SqlxResult<()> {
        sqlx::query!(
            r#"
            UPDATE import_progress SET status = 'completed', completed_at = ?2 WHERE id = ?1
            "#,
            id,
            completed_at
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn fail_import(&self, id: i64, error_message: &str) -> SqlxResult<()> {
        sqlx::query!(
            r#"
            UPDATE import_progress SET status = 'failed', error_message = ?2 WHERE id = ?1
            "#,
            id,
            error_message
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_import_progress(&self, id: i64) -> SqlxResult<()> {
        sqlx::query!("DELETE FROM import_progress WHERE id = ?1", id)
            .execute(&*self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_import_progress_by_status(
        &self,
        status: &str,
    ) -> SqlxResult<Vec<ImportProgress>> {
        sqlx::query_as::<_, ImportProgress>(
            r#"
            SELECT id, file_path, total_messages, processed_messages, status, started_at, completed_at, error_message
            FROM import_progress WHERE status = ?1 ORDER BY started_at DESC
            "#,
        )
        .bind(status)
        .fetch_all(&*self.pool)
        .await
    }

    pub async fn get_import_source_checkpoint(
        &self,
        source_kind: &str,
        source_path: &str,
    ) -> SqlxResult<Option<ImportSourceCheckpoint>> {
        sqlx::query_as::<_, ImportSourceCheckpoint>(
            r#"
            SELECT
                id,
                source_kind,
                source_path,
                fingerprint,
                file_size,
                modified_at,
                platform,
                chat_name,
                meta_id,
                last_processed_at,
                last_inserted_messages,
                last_duplicate_messages,
                status,
                error_message
            FROM import_source_checkpoint
            WHERE source_kind = ?1 AND source_path = ?2
            LIMIT 1
            "#,
        )
        .bind(source_kind)
        .bind(source_path)
        .fetch_optional(&*self.pool)
        .await
    }

    pub async fn source_checkpoint_is_unchanged(
        &self,
        source_kind: &str,
        source_path: &str,
        fingerprint: &str,
    ) -> SqlxResult<bool> {
        let row = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) AS count
            FROM import_source_checkpoint
            WHERE source_kind = ?1
              AND source_path = ?2
              AND fingerprint = ?3
              AND status = 'completed'
            "#,
        )
        .bind(source_kind)
        .bind(source_path)
        .bind(fingerprint)
        .fetch_one(&*self.pool)
        .await?;
        Ok(row > 0)
    }

    pub async fn upsert_import_source_checkpoint(
        &self,
        checkpoint: &ImportSourceCheckpoint,
    ) -> SqlxResult<()> {
        sqlx::query(
            r#"
            INSERT INTO import_source_checkpoint (
                source_kind,
                source_path,
                fingerprint,
                file_size,
                modified_at,
                platform,
                chat_name,
                meta_id,
                last_processed_at,
                last_inserted_messages,
                last_duplicate_messages,
                status,
                error_message
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ON CONFLICT(source_kind, source_path) DO UPDATE SET
                fingerprint = excluded.fingerprint,
                file_size = excluded.file_size,
                modified_at = excluded.modified_at,
                platform = excluded.platform,
                chat_name = excluded.chat_name,
                meta_id = excluded.meta_id,
                last_processed_at = excluded.last_processed_at,
                last_inserted_messages = excluded.last_inserted_messages,
                last_duplicate_messages = excluded.last_duplicate_messages,
                status = excluded.status,
                error_message = excluded.error_message
            "#,
        )
        .bind(&checkpoint.source_kind)
        .bind(&checkpoint.source_path)
        .bind(&checkpoint.fingerprint)
        .bind(checkpoint.file_size)
        .bind(checkpoint.modified_at)
        .bind(&checkpoint.platform)
        .bind(&checkpoint.chat_name)
        .bind(checkpoint.meta_id)
        .bind(checkpoint.last_processed_at)
        .bind(checkpoint.last_inserted_messages)
        .bind(checkpoint.last_duplicate_messages)
        .bind(&checkpoint.status)
        .bind(&checkpoint.error_message)
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_import_source_checkpoint_failed(
        &self,
        source_kind: &str,
        source_path: &str,
        error_message: &str,
        processed_at: i64,
    ) -> SqlxResult<()> {
        sqlx::query(
            r#"
            UPDATE import_source_checkpoint
            SET status = 'failed',
                error_message = ?3,
                last_processed_at = ?4
            WHERE source_kind = ?1 AND source_path = ?2
            "#,
        )
        .bind(source_kind)
        .bind(source_path)
        .bind(error_message)
        .bind(processed_at)
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    // Conversations methods
    pub async fn create_conversation(&self, conversation: &Conversations) -> SqlxResult<i64> {
        let result = sqlx::query!(
            r#"
            INSERT INTO conversations (session_id, messages, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            conversation.session_id,
            conversation.messages,
            conversation.created_at,
            conversation.updated_at
        )
        .execute(&*self.pool)
        .await?;
        Ok(result.last_insert_rowid())
    }

    pub async fn get_conversation(&self, id: i64) -> SqlxResult<Option<Conversations>> {
        sqlx::query_as!(
            Conversations,
            r#"
            SELECT id, session_id, messages, created_at, updated_at
            FROM conversations WHERE id = ?1
            "#,
            id
        )
        .fetch_optional(&*self.pool)
        .await
    }

    pub async fn update_conversation(&self, conversation: &Conversations) -> SqlxResult<()> {
        sqlx::query!(
            r#"
            UPDATE conversations SET session_id = ?2, messages = ?3, updated_at = ?4 WHERE id = ?1
            "#,
            conversation.id,
            conversation.session_id,
            conversation.messages,
            conversation.updated_at
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_conversation(&self, id: i64) -> SqlxResult<()> {
        sqlx::query!("DELETE FROM conversations WHERE id = ?1", id)
            .execute(&*self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_conversations_by_session_id(
        &self,
        session_id: &str,
    ) -> SqlxResult<Vec<Conversations>> {
        sqlx::query_as::<_, Conversations>(
            r#"
            SELECT id, session_id, messages, created_at, updated_at
            FROM conversations WHERE session_id = ?1 ORDER BY created_at DESC
            "#,
        )
        .bind(session_id)
        .fetch_all(&*self.pool)
        .await
    }

    /// 获取每小时活跃度分布（带时间过滤）
    pub async fn get_hourly_activity_with_filter(
        &self,
        meta_id: i64,
        filter: Option<TimeFilter>,
    ) -> SqlxResult<Vec<TimeActivity>> {
        // 构建时间过滤条件
        let (time_clause, mut params) = if let Some(ref f) = filter {
            self.build_time_filter(f, Some("msg"))
        } else {
            (String::new(), Vec::new())
        };

        // 添加 meta_id 条件
        let meta_condition = " WHERE msg.meta_id = ?".to_string();
        let mut clause = meta_condition;
        if !time_clause.is_empty() {
            clause.push_str(" AND ");
            clause.push_str(&time_clause.trim_start_matches(" WHERE "));
        }
        params.insert(0, meta_id);

        // 构建排除系统消息的过滤条件
        let clause_with_system = self.build_system_message_filter(&clause);

        // 查询每小时活跃度
        let query = format!(
            r#"
            SELECT
                CAST(strftime('%H', msg.ts, 'unixepoch', 'localtime') AS INTEGER) as period,
                COUNT(*) as message_count
            FROM message msg
            JOIN member m ON msg.sender_id = m.id
            {}
            GROUP BY period
            ORDER BY period
            "#,
            clause_with_system
        );

        #[derive(Debug, sqlx::FromRow)]
        struct HourlyRow {
            period: i64,
            message_count: i64,
        }

        let rows: Vec<HourlyRow> = Self::bind_i64_params(sqlx::query_as(&query), &params)
            .fetch_all(&*self.pool)
            .await?;

        // 确保返回0-23所有小时
        let mut result = Vec::with_capacity(24);
        for hour in 0..24 {
            if let Some(row) = rows.iter().find(|r| r.period == hour) {
                result.push(TimeActivity {
                    period: hour,
                    message_count: row.message_count,
                });
            } else {
                result.push(TimeActivity {
                    period: hour,
                    message_count: 0,
                });
            }
        }

        Ok(result)
    }
    /// 获取每日活跃度趋势（带时间过滤）
    pub async fn get_daily_activity_with_filter(
        &self,
        meta_id: i64,
        filter: Option<TimeFilter>,
    ) -> SqlxResult<Vec<TimeActivity>> {
        // 构建时间过滤条件
        let (time_clause, mut params) = if let Some(ref f) = filter {
            self.build_time_filter(f, Some("msg"))
        } else {
            (String::new(), Vec::new())
        };

        // 添加 meta_id 条件
        let meta_condition = " WHERE msg.meta_id = ?".to_string();
        let mut clause = meta_condition;
        if !time_clause.is_empty() {
            clause.push_str(" AND ");
            clause.push_str(&time_clause.trim_start_matches(" WHERE "));
        }
        params.insert(0, meta_id);

        // 构建排除系统消息的过滤条件
        let clause_with_system = self.build_system_message_filter(&clause);

        // 查询每日活跃度
        let query = format!(
            r#"
            SELECT
                CAST(strftime('%Y%m%d', msg.ts, 'unixepoch', 'localtime') AS INTEGER) as period,
                COUNT(*) as message_count
            FROM message msg
            JOIN member m ON msg.sender_id = m.id
            {}
            GROUP BY period
            ORDER BY period
            "#,
            clause_with_system
        );

        #[derive(Debug, sqlx::FromRow)]
        struct DailyRow {
            period: i64,
            message_count: i64,
        }

        let rows: Vec<DailyRow> = Self::bind_i64_params(sqlx::query_as(&query), &params)
            .fetch_all(&*self.pool)
            .await?;

        let result = rows
            .into_iter()
            .map(|row| TimeActivity {
                period: row.period,
                message_count: row.message_count,
            })
            .collect();

        Ok(result)
    }
    /// 获取星期活跃度分布（带时间过滤）
    pub async fn get_weekday_activity_with_filter(
        &self,
        meta_id: i64,
        filter: Option<TimeFilter>,
    ) -> SqlxResult<Vec<TimeActivity>> {
        // 构建时间过滤条件
        let (time_clause, mut params) = if let Some(ref f) = filter {
            self.build_time_filter(f, Some("msg"))
        } else {
            (String::new(), Vec::new())
        };

        // 添加 meta_id 条件
        let meta_condition = " WHERE msg.meta_id = ?".to_string();
        let mut clause = meta_condition;
        if !time_clause.is_empty() {
            clause.push_str(" AND ");
            clause.push_str(&time_clause.trim_start_matches(" WHERE "));
        }
        params.insert(0, meta_id);

        // 构建排除系统消息的过滤条件
        let clause_with_system = self.build_system_message_filter(&clause);

        // 查询星期活跃度
        let query = format!(
            r#"
            SELECT
                CASE
                  WHEN CAST(strftime('%w', msg.ts, 'unixepoch', 'localtime') AS INTEGER) = 0 THEN 7
                  ELSE CAST(strftime('%w', msg.ts, 'unixepoch', 'localtime') AS INTEGER)
                END as period,
                COUNT(*) as message_count
            FROM message msg
            JOIN member m ON msg.sender_id = m.id
            {}
            GROUP BY period
            ORDER BY period
            "#,
            clause_with_system
        );

        #[derive(Debug, sqlx::FromRow)]
        struct WeekdayRow {
            period: i64,
            message_count: i64,
        }

        let rows: Vec<WeekdayRow> = Self::bind_i64_params(sqlx::query_as(&query), &params)
            .fetch_all(&*self.pool)
            .await?;

        // 确保返回1-7所有星期
        let mut result = Vec::with_capacity(7);
        for weekday in 1..=7 {
            if let Some(row) = rows.iter().find(|r| r.period == weekday) {
                result.push(TimeActivity {
                    period: weekday,
                    message_count: row.message_count,
                });
            } else {
                result.push(TimeActivity {
                    period: weekday,
                    message_count: 0,
                });
            }
        }

        Ok(result)
    }

    /// 获取月份活跃度分布（带时间过滤）
    pub async fn get_monthly_activity_with_filter(
        &self,
        meta_id: i64,
        filter: Option<TimeFilter>,
    ) -> SqlxResult<Vec<TimeActivity>> {
        // 构建时间过滤条件
        let (time_clause, mut params) = if let Some(ref f) = filter {
            self.build_time_filter(f, Some("msg"))
        } else {
            (String::new(), Vec::new())
        };

        // 添加 meta_id 条件
        let meta_condition = " WHERE msg.meta_id = ?".to_string();
        let mut clause = meta_condition;
        if !time_clause.is_empty() {
            clause.push_str(" AND ");
            clause.push_str(&time_clause.trim_start_matches(" WHERE "));
        }
        params.insert(0, meta_id);

        // 构建排除系统消息的过滤条件
        let clause_with_system = self.build_system_message_filter(&clause);

        // 查询月份活跃度
        let query = format!(
            r#"
            SELECT
                CAST(strftime('%m', msg.ts, 'unixepoch', 'localtime') AS INTEGER) as period,
                COUNT(*) as message_count
            FROM message msg
            JOIN member m ON msg.sender_id = m.id
            {}
            GROUP BY period
            ORDER BY period
            "#,
            clause_with_system
        );

        #[derive(Debug, sqlx::FromRow)]
        struct MonthlyRow {
            period: i64,
            message_count: i64,
        }

        let rows: Vec<MonthlyRow> = Self::bind_i64_params(sqlx::query_as(&query), &params)
            .fetch_all(&*self.pool)
            .await?;

        // 确保返回1-12所有月份
        let mut result = Vec::with_capacity(12);
        for month in 1..=12 {
            if let Some(row) = rows.iter().find(|r| r.period == month) {
                result.push(TimeActivity {
                    period: month,
                    message_count: row.message_count,
                });
            } else {
                result.push(TimeActivity {
                    period: month,
                    message_count: 0,
                });
            }
        }

        Ok(result)
    }
    /// 获取年份活跃度分布（带时间过滤）
    pub async fn get_yearly_activity_with_filter(
        &self,
        meta_id: i64,
        filter: Option<TimeFilter>,
    ) -> SqlxResult<Vec<TimeActivity>> {
        // 构建时间过滤条件
        let (time_clause, mut params) = if let Some(ref f) = filter {
            self.build_time_filter(f, Some("msg"))
        } else {
            (String::new(), Vec::new())
        };

        // 添加 meta_id 条件
        let meta_condition = " WHERE msg.meta_id = ?".to_string();
        let mut clause = meta_condition;
        if !time_clause.is_empty() {
            clause.push_str(" AND ");
            clause.push_str(&time_clause.trim_start_matches(" WHERE "));
        }
        params.insert(0, meta_id);

        // 构建排除系统消息的过滤条件
        let clause_with_system = self.build_system_message_filter(&clause);

        // 查询年份活跃度
        let query = format!(
            r#"
            SELECT
                CAST(strftime('%Y', msg.ts, 'unixepoch', 'localtime') AS INTEGER) as period,
                COUNT(*) as message_count
            FROM message msg
            JOIN member m ON msg.sender_id = m.id
            {}
            GROUP BY period
            ORDER BY period
            "#,
            clause_with_system
        );

        #[derive(Debug, sqlx::FromRow)]
        struct YearlyRow {
            period: i64,
            message_count: i64,
        }

        let rows: Vec<YearlyRow> = Self::bind_i64_params(sqlx::query_as(&query), &params)
            .fetch_all(&*self.pool)
            .await?;

        let result = rows
            .into_iter()
            .map(|row| TimeActivity {
                period: row.period,
                message_count: row.message_count,
            })
            .collect();

        Ok(result)
    }

    /// 获取消息类型分布（带时间过滤）
    pub async fn get_message_type_distribution_with_filter(
        &self,
        meta_id: i64,
        filter: Option<TimeFilter>,
    ) -> SqlxResult<Vec<MessageTypeDistribution>> {
        // 构建时间过滤条件
        let (time_clause, mut params) = if let Some(ref f) = filter {
            self.build_time_filter(f, Some("msg"))
        } else {
            (String::new(), Vec::new())
        };

        // 添加 meta_id 条件
        let meta_condition = " WHERE msg.meta_id = ?".to_string();
        let mut clause = meta_condition;
        if !time_clause.is_empty() {
            clause.push_str(" AND ");
            clause.push_str(&time_clause.trim_start_matches(" WHERE "));
        }
        params.insert(0, meta_id);

        // 构建排除系统消息的过滤条件
        let clause_with_system = self.build_system_message_filter(&clause);

        // 查询消息类型分布
        let query = format!(
            r#"
            SELECT
                msg.msg_type as msg_type,
                COUNT(*) as count
            FROM message msg
            JOIN member m ON msg.sender_id = m.id
            {}
            GROUP BY msg.msg_type
            ORDER BY count DESC
            "#,
            clause_with_system
        );

        #[derive(Debug, sqlx::FromRow)]
        struct TypeRow {
            msg_type: i64,
            count: i64,
        }

        let rows: Vec<TypeRow> = Self::bind_i64_params(sqlx::query_as(&query), &params)
            .fetch_all(&*self.pool)
            .await?;

        let result = rows
            .into_iter()
            .map(|row| MessageTypeDistribution {
                msg_type: row.msg_type,
                count: row.count,
            })
            .collect();

        Ok(result)
    }

    /// 获取消息长度分布（带时间过滤，仅统计文字消息）
    pub async fn get_message_length_distribution_with_filter(
        &self,
        meta_id: i64,
        filter: Option<TimeFilter>,
    ) -> SqlxResult<MessageLengthDistributionResult> {
        // 构建时间过滤条件
        let (time_clause, mut params) = if let Some(ref f) = filter {
            self.build_time_filter(f, Some("msg"))
        } else {
            (String::new(), Vec::new())
        };

        // 添加 meta_id 条件
        let meta_condition = " WHERE msg.meta_id = ?".to_string();
        let mut clause = meta_condition;
        if !time_clause.is_empty() {
            clause.push_str(" AND ");
            clause.push_str(&time_clause.trim_start_matches(" WHERE "));
        }
        params.insert(0, meta_id);

        // 构建排除系统消息的过滤条件
        let clause_with_system = self.build_system_message_filter(&clause);

        // 只统计文字消息 (type = 0)，并且 content 不为空且长度大于0
        let type_condition = if clause_with_system.contains("WHERE") {
            format!(
                "{} AND msg.msg_type = 0 AND msg.content IS NOT NULL AND LENGTH(msg.content) > 0",
                clause_with_system
            )
        } else {
            format!(
                "WHERE msg.msg_type = 0 AND msg.content IS NOT NULL AND LENGTH(msg.content) > 0"
            )
        };

        // 查询消息长度分布
        let query = format!(
            r#"
            SELECT
                LENGTH(msg.content) as len,
                COUNT(*) as count
            FROM message msg
            JOIN member m ON msg.sender_id = m.id
            {}
            GROUP BY len
            ORDER BY len
            "#,
            type_condition
        );

        #[derive(Debug, sqlx::FromRow)]
        struct LengthRow {
            len: i64,
            count: i64,
        }

        let rows: Vec<LengthRow> = Self::bind_i64_params(sqlx::query_as(&query), &params)
            .fetch_all(&*self.pool)
            .await?;

        // 构建 detail：1-25 逐字
        let mut detail = Vec::new();
        for i in 1..=25 {
            let found = rows.iter().find(|r| r.len == i);
            detail.push(MessageLengthDistributionDetail {
                len: i,
                count: found.map(|r| r.count).unwrap_or(0),
            });
        }

        // 构建 grouped：每5字一段（与Xenobot相同范围）
        let ranges5 = [
            (1, 5, "1-5".to_string()),
            (6, 10, "6-10".to_string()),
            (11, 15, "11-15".to_string()),
            (16, 20, "16-20".to_string()),
            (21, 25, "21-25".to_string()),
            (26, 30, "26-30".to_string()),
            (31, 35, "31-35".to_string()),
            (36, 40, "36-40".to_string()),
            (41, 45, "41-45".to_string()),
            (46, 50, "46-50".to_string()),
            (51, 60, "51-60".to_string()),
            (61, 70, "61-70".to_string()),
            (71, 80, "71-80".to_string()),
            (81, 100, "81-100".to_string()),
            (101, i64::MAX, "100+".to_string()),
        ];

        let mut grouped = Vec::new();
        for (min, max, label) in ranges5.iter() {
            let count: i64 = rows
                .iter()
                .filter(|r| r.len >= *min && r.len <= *max)
                .map(|r| r.count)
                .sum();
            grouped.push(MessageLengthDistributionGrouped {
                range: label.clone(),
                count,
            });
        }

        Ok(MessageLengthDistributionResult { detail, grouped })
    }

    /// 获取时间范围（最早和最晚消息时间戳）
    pub async fn get_time_range(&self, meta_id: i64) -> SqlxResult<Option<TimeRange>> {
        let query = r#"
            SELECT MIN(ts) as earliest, MAX(ts) as latest
            FROM message
            WHERE meta_id = ?1
        "#;

        #[derive(Debug, sqlx::FromRow)]
        struct TimeRangeRow {
            earliest: Option<i64>,
            latest: Option<i64>,
        }

        let row: Option<TimeRangeRow> = sqlx::query_as(query)
            .bind(meta_id)
            .fetch_optional(&*self.pool)
            .await?;

        match row {
            Some(r) => Ok(Some(TimeRange {
                earliest: r.earliest,
                latest: r.latest,
            })),
            None => Ok(None),
        }
    }

    /// 获取会话中可用的年份列表
    pub async fn get_available_years(&self, meta_id: i64) -> SqlxResult<Vec<i64>> {
        let query = r#"
            SELECT DISTINCT CAST(strftime('%Y', msg.ts, 'unixepoch', 'localtime') AS INTEGER) as year
            FROM message msg
            JOIN member m ON msg.sender_id = m.id
            WHERE msg.meta_id = ?1 AND COALESCE(m.account_name, '') != '系统消息'
            ORDER BY year
        "#;

        #[derive(Debug, sqlx::FromRow)]
        struct YearRow {
            year: i64,
        }

        let rows: Vec<YearRow> = sqlx::query_as(query)
            .bind(meta_id)
            .fetch_all(&*self.pool)
            .await?;

        Ok(rows.into_iter().map(|r| r.year).collect())
    }
}
