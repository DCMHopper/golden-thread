use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadSummary {
    pub id: String,
    pub name: Option<String>,
    pub last_message_at: Option<i64>,
    pub message_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRow {
    pub id: String,
    pub thread_id: String,
    pub sender_id: Option<String>,
    pub sent_at: Option<i64>,
    pub received_at: Option<i64>,
    pub message_type: String,
    pub body: Option<String>,
    pub is_outgoing: bool,
    pub is_view_once: bool,
    pub quote_message_id: Option<String>,
    pub metadata_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub message: MessageRow,
    pub rank: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactionSummary {
    pub message_id: String,
    pub emoji: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaRow {
    pub id: String,
    pub message_id: String,
    pub sha256: String,
    pub mime: Option<String>,
    pub size_bytes: Option<i64>,
    pub original_filename: Option<String>,
    pub kind: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadMediaRow {
    pub id: String,
    pub message_id: String,
    pub thread_id: String,
    pub sha256: String,
    pub mime: Option<String>,
    pub size_bytes: Option<i64>,
    pub original_filename: Option<String>,
    pub kind: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_ms: Option<i64>,
    pub sent_at: Option<i64>,
    pub received_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveStats {
    pub threads: i64,
    pub messages: i64,
    pub recipients: i64,
    pub attachments: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub color: String,
    pub created_at: i64,
    pub display_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageTags {
    pub message_id: String,
    pub tags: Vec<Tag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrapbookMessage {
    pub message: MessageRow,
    pub thread_name: Option<String>,
    pub is_discontinuous: bool,
}
