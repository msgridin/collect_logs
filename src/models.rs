use chrono::{DateTime, Utc};
use serde::Serialize;


#[derive(Debug, Serialize)]
pub struct LogRecord {
    pub id: i64,
    pub date: DateTime<Utc>,
    pub user: String,
    pub comp: String,
    pub app: String,
    pub event: String,
    pub comment: String,
    pub transaction_id: i64,
    pub transaction_status: String,
    pub metadata: String,
    pub data: String,
    pub error: bool,
    pub database: String,
    pub server: String,
    pub status: String,
}

#[derive(Debug)]
pub struct BaseOptions {
    pub server: String,
    pub name: String,
    pub start_log_record: i64,
    pub end_log_record: i64,
    pub log: String,
}
