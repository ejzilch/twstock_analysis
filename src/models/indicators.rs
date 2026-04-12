use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeIndicatorsRequest {
    pub request_id: String,
    pub symbol: String,
    pub from_ms: i64,
    pub to_ms: i64,
    pub interval: String,
    pub indicators: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeIndicatorsResponse {
    pub symbol: String,
    pub interval: String,
    pub from_ms: i64,
    pub to_ms: i64,
    pub indicators: HashMap<String, serde_json::Value>,
    pub computed_at_ms: i64,
    pub computation_time_ms: i64,
    pub cached: bool,
    pub dag_execution_order: Vec<String>,
}
