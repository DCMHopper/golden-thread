use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    pub cmd: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub id: u64,
    pub ok: bool,
    pub payload: Option<serde_json::Value>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ThumbRequest {
    pub sha256: String,
    pub max_size: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MediaRequest {
    pub sha256: String,
    pub mime: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DataUrlRequest {
    pub sha256: String,
    pub mime: String,
    pub max_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ThumbResponse {
    pub data_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MediaResponse {
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DataUrlResponse {
    pub data_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EvictionsResponse {
    pub sha256s: Vec<String>,
}
