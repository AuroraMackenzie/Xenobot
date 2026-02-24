use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Platform types supported by Xenobot.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Platform {
    /// WeChat application.
    WeChat,

    /// WhatsApp application.
    WhatsApp,

    /// LINE application.
    Line,

    /// Telegram application.
    Telegram,

    /// QQ application.
    Qq,

    /// Discord application.
    Discord,

    /// Instagram application.
    Instagram,

    /// iMessage application.
    IMessage,

    /// Facebook Messenger application.
    Messenger,

    /// KakaoTalk application.
    KakaoTalk,

    /// Slack application.
    Slack,

    /// Microsoft Teams application.
    Teams,

    /// Signal application.
    Signal,

    /// Custom platform.
    Custom(String),
}

/// Message type enumeration.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MessageType {
    /// Text message.
    Text,

    /// Image message.
    Image,

    /// Video message.
    Video,

    /// Audio message.
    Audio,

    /// File message.
    File,

    /// Location message.
    Location,

    /// Sticker message.
    Sticker,

    /// Voice message.
    Voice,

    /// System message.
    System,

    /// Link message.
    Link,

    /// Quote message.
    Quote,

    /// Forwarded message.
    Forwarded,

    /// Reply message.
    Reply,

    /// Call message.
    Call,

    /// Video call message.
    VideoCall,

    /// Payment message.
    Payment,

    /// Red envelope message.
    RedEnvelope,

    /// Contact card message.
    ContactCard,

    /// Unknown message type.
    Unknown,
}

/// Message status.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MessageStatus {
    /// Message sent successfully.
    Sent,

    /// Message delivered to recipient.
    Delivered,

    /// Message read by recipient.
    Read,

    /// Message sending failed.
    Failed,

    /// Message pending.
    Pending,

    /// Message recalled.
    Recalled,

    /// Message deleted.
    Deleted,

    /// Unknown status.
    Unknown,
}

/// Media information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaInfo {
    /// Media type.
    pub media_type: MediaType,

    /// File path or URL.
    pub path: PathBuf,

    /// File size in bytes.
    pub size: u64,

    /// MIME type.
    pub mime_type: String,

    /// Dimensions for image/video.
    pub dimensions: Option<(u32, u32)>,

    /// Duration for audio/video.
    pub duration: Option<u64>,

    /// Thumbnail path.
    pub thumbnail: Option<PathBuf>,

    /// Encryption key for encrypted media.
    pub encryption_key: Option<Vec<u8>>,

    /// Checksum for integrity verification.
    pub checksum: Option<String>,
}

/// Media type enumeration.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MediaType {
    /// Image media.
    Image,

    /// Video media.
    Video,

    /// Audio media.
    Audio,

    /// Document media.
    Document,

    /// Sticker media.
    Sticker,

    /// Animation media.
    Animation,

    /// Voice message media.
    Voice,

    /// Unknown media type.
    Unknown,
}

/// Chat message structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique message ID.
    pub id: String,

    /// Platform this message belongs to.
    pub platform: Platform,

    /// Chat ID or conversation ID.
    pub chat_id: String,

    /// Sender ID.
    pub sender_id: String,

    /// Sender display name.
    pub sender_name: String,

    /// Recipient ID (for private chat) or group ID.
    pub recipient_id: String,

    /// Recipient display name.
    pub recipient_name: String,

    /// Message timestamp in milliseconds.
    pub timestamp: i64,

    /// Message type.
    pub message_type: MessageType,

    /// Message content (text or media info).
    pub content: MessageContent,

    /// Message status.
    pub status: MessageStatus,

    /// Is this message edited?
    pub edited: bool,

    /// Edit timestamp in milliseconds (if edited).
    pub edit_timestamp: Option<i64>,

    /// Is this message forwarded?
    pub forwarded: bool,

    /// Forward source info (if forwarded).
    pub forward_source: Option<ForwardSource>,

    /// Is this message a reply?
    pub reply: bool,

    /// Reply to message ID (if reply).
    pub reply_to_id: Option<String>,

    /// Message metadata.
    pub metadata: serde_json::Value,

    /// Platform-specific raw data.
    pub raw_data: Option<serde_json::Value>,

    /// Message processing flags.
    pub flags: Vec<MessageFlag>,
}

/// Message content enumeration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageContent {
    /// Text content.
    Text(String),

    /// Media content.
    Media(MediaInfo),

    /// Location content.
    Location(LocationInfo),

    /// Contact card content.
    ContactCard(ContactInfo),

    /// System notification content.
    System(String),

    /// Mixed content (text + media).
    Mixed {
        /// Text content.
        text: String,

        /// Media info.
        media: MediaInfo,
    },

    /// Empty content (e.g., system message).
    Empty,
}

/// Location information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationInfo {
    /// Latitude.
    pub latitude: f64,

    /// Longitude.
    pub longitude: f64,

    /// Location name or address.
    pub name: Option<String>,

    /// Accuracy in meters.
    pub accuracy: Option<f64>,

    /// Altitude in meters.
    pub altitude: Option<f64>,

    /// Timestamp of location.
    pub timestamp: Option<i64>,
}

/// Contact information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactInfo {
    /// Contact ID.
    pub contact_id: String,

    /// Contact name.
    pub name: String,

    /// Contact phone number.
    pub phone_number: Option<String>,

    /// Contact email.
    pub email: Option<String>,

    /// Contact avatar URL.
    pub avatar_url: Option<String>,

    /// Contact organization.
    pub organization: Option<String>,

    /// Contact title.
    pub title: Option<String>,

    /// Contact notes.
    pub notes: Option<String>,
}

/// Forward source information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForwardSource {
    /// Original message ID.
    pub original_message_id: String,

    /// Original sender ID.
    pub original_sender_id: String,

    /// Original sender name.
    pub original_sender_name: String,

    /// Original chat ID.
    pub original_chat_id: String,

    /// Original platform.
    pub original_platform: Platform,

    /// Original timestamp.
    pub original_timestamp: i64,
}

/// Message flag enumeration.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MessageFlag {
    /// Message is important.
    Important,

    /// Message contains sensitive content.
    Sensitive,

    /// Message is spam.
    Spam,

    /// Message is deleted.
    Deleted,

    /// Message is hidden.
    Hidden,

    /// Message is pinned.
    Pinned,

    /// Message has been processed.
    Processed,

    /// Message needs review.
    NeedsReview,

    /// Message is encrypted.
    Encrypted,

    /// Message is archived.
    Archived,
}

/// Chat type enumeration.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChatType {
    /// Private one-on-one chat.
    Private,

    /// Group chat.
    Group,

    /// Channel (broadcast).
    Channel,

    /// Supergroup (large group).
    Supergroup,

    /// Community (multiple groups).
    Community,

    /// Broadcast list.
    Broadcast,
}

/// Chat information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chat {
    /// Unique chat ID.
    pub id: String,

    /// Platform this chat belongs to.
    pub platform: Platform,

    /// Chat type.
    pub chat_type: ChatType,

    /// Chat display name.
    pub name: String,

    /// Chat description.
    pub description: Option<String>,

    /// Chat avatar URL or path.
    pub avatar: Option<String>,

    /// Chat creation timestamp.
    pub created_at: i64,

    /// Chat last activity timestamp.
    pub last_activity: i64,

    /// Number of participants.
    pub participant_count: u32,

    /// Is this chat archived?
    pub archived: bool,

    /// Is this chat muted?
    pub muted: bool,

    /// Is this chat pinned?
    pub pinned: bool,

    /// Chat metadata.
    pub metadata: serde_json::Value,

    /// Platform-specific raw data.
    pub raw_data: Option<serde_json::Value>,
}

/// User information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// Unique user ID.
    pub id: String,

    /// Platform this user belongs to.
    pub platform: Platform,

    /// User display name.
    pub name: String,

    /// User nickname or alias.
    pub nickname: Option<String>,

    /// User avatar URL or path.
    pub avatar: Option<String>,

    /// User status.
    pub status: UserStatus,

    /// User presence (online/offline).
    pub presence: UserPresence,

    /// User last seen timestamp.
    pub last_seen: Option<i64>,

    /// User registration timestamp.
    pub registered_at: Option<i64>,

    /// User metadata.
    pub metadata: serde_json::Value,

    /// Platform-specific raw data.
    pub raw_data: Option<serde_json::Value>,
}

/// User status enumeration.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UserStatus {
    /// Active user.
    Active,

    /// Inactive user.
    Inactive,

    /// Banned user.
    Banned,

    /// Deleted user.
    Deleted,

    /// Suspended user.
    Suspended,

    /// Unknown status.
    Unknown,
}

/// User presence enumeration.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UserPresence {
    /// User is online.
    Online,

    /// User is offline.
    Offline,

    /// User is away.
    Away,

    /// User is busy.
    Busy,

    /// User is idle.
    Idle,

    /// User is invisible.
    Invisible,

    /// Unknown presence.
    Unknown,
}

/// Analysis result types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnalysisResult {
    /// Statistical analysis.
    Statistics(StatisticalAnalysis),

    /// Time-based analysis.
    TimeAnalysis(TimeAnalysis),

    /// User activity analysis.
    UserActivity(UserActivityAnalysis),

    /// Content analysis.
    ContentAnalysis(ContentAnalysis),

    /// Network analysis.
    NetworkAnalysis(NetworkAnalysis),

    /// Sentiment analysis.
    SentimentAnalysis(SentimentAnalysis),

    /// Topic analysis.
    TopicAnalysis(TopicAnalysis),

    /// Behavior analysis.
    BehaviorAnalysis(BehaviorAnalysis),
}

/// Statistical analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticalAnalysis {
    /// Total message count.
    pub total_messages: u64,

    /// Total participants.
    pub total_participants: u32,

    /// Average messages per day.
    pub avg_messages_per_day: f64,

    /// Most active user.
    pub most_active_user: String,

    /// Least active user.
    pub least_active_user: String,

    /// Message type distribution.
    pub message_type_distribution: Vec<(MessageType, u64)>,

    /// Time period of analysis.
    pub time_period: (i64, i64),
}

/// Time analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeAnalysis {
    /// Hourly activity distribution.
    pub hourly_activity: Vec<(u8, u64)>,

    /// Daily activity distribution.
    pub daily_activity: Vec<(String, u64)>,

    /// Weekly activity distribution.
    pub weekly_activity: Vec<(String, u64)>,

    /// Monthly activity distribution.
    pub monthly_activity: Vec<(String, u64)>,

    /// Yearly activity distribution.
    pub yearly_activity: Vec<(String, u64)>,

    /// Activity trends over time.
    pub activity_trends: Vec<(i64, u64)>,
}

/// User activity analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserActivityAnalysis {
    /// User activity ranking.
    pub user_ranking: Vec<(String, u64)>,

    /// User message type distribution.
    pub user_message_type_distribution: Vec<(String, Vec<(MessageType, u64)>)>,

    /// User activity patterns.
    pub user_activity_patterns: Vec<(String, Vec<(u8, f64)>)>,

    /// User response times.
    pub user_response_times: Vec<(String, f64)>,

    /// User engagement scores.
    pub user_engagement_scores: Vec<(String, f64)>,
}

/// Content analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentAnalysis {
    /// Word frequency analysis.
    pub word_frequency: Vec<(String, u64)>,

    /// Keyword extraction.
    pub keywords: Vec<String>,

    /// Topic modeling results.
    pub topics: Vec<Topic>,

    /// Named entity recognition.
    pub entities: Vec<Entity>,

    /// Sentiment distribution.
    pub sentiment_distribution: Vec<(Sentiment, f64)>,

    /// Language detection.
    pub languages: Vec<(String, f64)>,
}

/// Network analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkAnalysis {
    /// User interaction network.
    pub interaction_network: InteractionNetwork,

    /// Community detection.
    pub communities: Vec<Community>,

    /// Centrality measures.
    pub centrality: Vec<(String, CentralityMetrics)>,

    /// Path analysis.
    pub paths: Vec<Path>,
}

/// Sentiment analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentAnalysis {
    /// Overall sentiment score.
    pub overall_sentiment: f64,

    /// Sentiment over time.
    pub sentiment_over_time: Vec<(i64, f64)>,

    /// User sentiment scores.
    pub user_sentiment: Vec<(String, f64)>,

    /// Topic sentiment.
    pub topic_sentiment: Vec<(String, f64)>,
}

/// Topic analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicAnalysis {
    /// Detected topics.
    pub topics: Vec<Topic>,

    /// Topic evolution over time.
    pub topic_evolution: Vec<(i64, Vec<TopicWeight>)>,

    /// Topic-user association.
    pub topic_user_association: Vec<(String, Vec<TopicWeight>)>,
}

/// Behavior analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorAnalysis {
    /// Message patterns.
    pub message_patterns: Vec<MessagePattern>,

    /// User behavior clusters.
    pub behavior_clusters: Vec<BehaviorCluster>,

    /// Anomaly detection.
    pub anomalies: Vec<Anomaly>,

    /// Prediction models.
    pub predictions: Vec<Prediction>,
}

/// Topic structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Topic {
    /// Topic ID.
    pub id: String,

    /// Topic label.
    pub label: String,

    /// Topic keywords.
    pub keywords: Vec<String>,

    /// Topic weight.
    pub weight: f64,

    /// Topic coherence score.
    pub coherence: f64,
}

/// Entity structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    /// Entity text.
    pub text: String,

    /// Entity type.
    pub entity_type: EntityType,

    /// Entity start position.
    pub start: usize,

    /// Entity end position.
    pub end: usize,

    /// Entity confidence score.
    pub confidence: f64,
}

/// Entity type enumeration.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityType {
    /// Person entity.
    Person,

    /// Organization entity.
    Organization,

    /// Location entity.
    Location,

    /// Date entity.
    Date,

    /// Time entity.
    Time,

    /// Money entity.
    Money,

    /// Percentage entity.
    Percentage,

    /// Product entity.
    Product,

    /// Event entity.
    Event,

    /// Unknown entity type.
    Unknown,
}

/// Sentiment enumeration.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Sentiment {
    /// Very negative sentiment.
    VeryNegative,

    /// Negative sentiment.
    Negative,

    /// Neutral sentiment.
    Neutral,

    /// Positive sentiment.
    Positive,

    /// Very positive sentiment.
    VeryPositive,

    /// Mixed sentiment.
    Mixed,
}

/// Interaction network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionNetwork {
    /// Nodes (users).
    pub nodes: Vec<Node>,

    /// Edges (interactions).
    pub edges: Vec<Edge>,
}

/// Network node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// Node ID (user ID).
    pub id: String,

    /// Node label (user name).
    pub label: String,

    /// Node size (activity level).
    pub size: f64,

    /// Node color.
    pub color: String,

    /// Node properties.
    pub properties: serde_json::Value,
}

/// Network edge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// Source node ID.
    pub source: String,

    /// Target node ID.
    pub target: String,

    /// Edge weight (interaction strength).
    pub weight: f64,

    /// Edge label.
    pub label: String,

    /// Edge properties.
    pub properties: serde_json::Value,
}

/// Community structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Community {
    /// Community ID.
    pub id: String,

    /// Community label.
    pub label: String,

    /// Member user IDs.
    pub members: Vec<String>,

    /// Community size.
    pub size: usize,

    /// Community cohesion score.
    pub cohesion: f64,

    /// Community properties.
    pub properties: serde_json::Value,
}

/// Centrality metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CentralityMetrics {
    /// Degree centrality.
    pub degree: f64,

    /// Betweenness centrality.
    pub betweenness: f64,

    /// Closeness centrality.
    pub closeness: f64,

    /// Eigenvector centrality.
    pub eigenvector: f64,

    /// PageRank centrality.
    pub pagerank: f64,
}

/// Path structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Path {
    /// Source node ID.
    pub source: String,

    /// Target node ID.
    pub target: String,

    /// Path nodes.
    pub nodes: Vec<String>,

    /// Path length.
    pub length: usize,

    /// Path weight.
    pub weight: f64,
}

/// Topic weight.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicWeight {
    /// Topic ID.
    pub topic_id: String,

    /// Topic weight.
    pub weight: f64,
}

/// Message pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagePattern {
    /// Pattern ID.
    pub id: String,

    /// Pattern description.
    pub description: String,

    /// Pattern frequency.
    pub frequency: u64,

    /// Pattern confidence.
    pub confidence: f64,

    /// Pattern conditions.
    pub conditions: Vec<PatternCondition>,
}

/// Pattern condition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternCondition {
    /// Condition type.
    pub condition_type: PatternConditionType,

    /// Condition value.
    pub value: serde_json::Value,

    /// Condition operator.
    pub operator: PatternOperator,
}

/// Pattern condition type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PatternConditionType {
    /// Time-based condition.
    Time,

    /// User-based condition.
    User,

    /// Content-based condition.
    Content,

    /// Message type condition.
    MessageType,

    /// Platform condition.
    Platform,

    /// Custom condition.
    Custom,
}

/// Pattern operator.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PatternOperator {
    /// Equality operator.
    Equals,

    /// Not equals operator.
    NotEquals,

    /// Greater than operator.
    GreaterThan,

    /// Less than operator.
    LessThan,

    /// Greater than or equal operator.
    GreaterThanOrEqual,

    /// Less than or equal operator.
    LessThanOrEqual,

    /// Contains operator.
    Contains,

    /// Not contains operator.
    NotContains,

    /// Starts with operator.
    StartsWith,

    /// Ends with operator.
    EndsWith,

    /// Matches regex operator.
    MatchesRegex,

    /// In operator.
    In,

    /// Not in operator.
    NotIn,
}

/// Behavior cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorCluster {
    /// Cluster ID.
    pub id: String,

    /// Cluster label.
    pub label: String,

    /// Cluster centroid.
    pub centroid: Vec<f64>,

    /// Cluster size.
    pub size: usize,

    /// Cluster members.
    pub members: Vec<String>,

    /// Cluster properties.
    pub properties: serde_json::Value,
}

/// Anomaly detection result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    /// Anomaly ID.
    pub id: String,

    /// Anomaly type.
    pub anomaly_type: AnomalyType,

    /// Anomaly score.
    pub score: f64,

    /// Anomaly timestamp.
    pub timestamp: i64,

    /// Affected user IDs.
    pub affected_users: Vec<String>,

    /// Anomaly description.
    pub description: String,

    /// Anomaly severity.
    pub severity: AnomalySeverity,
}

/// Anomaly type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AnomalyType {
    /// Activity anomaly.
    Activity,

    /// Content anomaly.
    Content,

    /// Network anomaly.
    Network,

    /// Temporal anomaly.
    Temporal,

    /// Behavioral anomaly.
    Behavioral,

    /// Security anomaly.
    Security,

    /// Custom anomaly.
    Custom,
}

/// Anomaly severity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AnomalySeverity {
    /// Low severity.
    Low,

    /// Medium severity.
    Medium,

    /// High severity.
    High,

    /// Critical severity.
    Critical,
}

/// Prediction result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prediction {
    /// Prediction ID.
    pub id: String,

    /// Prediction type.
    pub prediction_type: PredictionType,

    /// Predicted value.
    pub predicted_value: serde_json::Value,

    /// Prediction confidence.
    pub confidence: f64,

    /// Prediction timestamp.
    pub timestamp: i64,

    /// Prediction horizon.
    pub horizon: PredictionHorizon,

    /// Prediction explanation.
    pub explanation: String,
}

/// Prediction type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PredictionType {
    /// Activity prediction.
    Activity,

    /// Sentiment prediction.
    Sentiment,

    /// User behavior prediction.
    UserBehavior,

    /// Topic prediction.
    Topic,

    /// Network evolution prediction.
    NetworkEvolution,

    /// Anomaly prediction.
    Anomaly,

    /// Custom prediction.
    Custom,
}

/// Prediction horizon.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PredictionHorizon {
    /// Short-term prediction (hours).
    ShortTerm,

    /// Medium-term prediction (days).
    MediumTerm,

    /// Long-term prediction (weeks/months).
    LongTerm,

    /// Custom horizon.
    Custom(i64),
}

/// Real-time event types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RealTimeEvent {
    /// New message event.
    NewMessage(Message),

    /// Message update event.
    MessageUpdate {
        /// Message ID.
        message_id: String,

        /// Updated fields.
        updates: serde_json::Value,
    },

    /// Message delete event.
    MessageDelete {
        /// Message ID.
        message_id: String,
    },

    /// User status update event.
    UserStatusUpdate {
        /// User ID.
        user_id: String,

        /// New status.
        new_status: UserStatus,

        /// New presence.
        new_presence: UserPresence,
    },

    /// Chat update event.
    ChatUpdate {
        /// Chat ID.
        chat_id: String,

        /// Updated fields.
        updates: serde_json::Value,
    },

    /// Platform connection event.
    PlatformConnection {
        /// Platform.
        platform: Platform,

        /// Connection status.
        status: ConnectionStatus,
    },

    /// Analysis update event.
    AnalysisUpdate {
        /// Analysis type.
        analysis_type: String,

        /// Updated results.
        results: AnalysisResult,
    },

    /// System alert event.
    SystemAlert {
        /// Alert level.
        level: AlertLevel,

        /// Alert message.
        message: String,

        /// Alert details.
        details: serde_json::Value,
    },
}

/// Connection status.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConnectionStatus {
    /// Connected.
    Connected,

    /// Disconnected.
    Disconnected,

    /// Connecting.
    Connecting,

    /// Error.
    Error(String),

    /// Reconnecting.
    Reconnecting,
}

/// Alert level.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AlertLevel {
    /// Information alert.
    Info,

    /// Warning alert.
    Warning,

    /// Error alert.
    Error,

    /// Critical alert.
    Critical,
}
