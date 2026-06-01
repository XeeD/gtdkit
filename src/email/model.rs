use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub(crate) struct Manifest {
    pub(crate) schema_version: u8,
    pub(crate) created_at: String,
    pub(crate) account: String,
    pub(crate) gmail_query: String,
    pub(crate) ordering: String,
    pub(crate) session_dir: String,
    pub(crate) newsletter_whitelist: String,
    pub(crate) knowledge_base_config: String,
    pub(crate) long_term_memory: String,
    pub(crate) contract: BTreeMap<String, bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub(crate) struct Queue {
    pub(crate) schema_version: u8,
    pub(crate) created_at: String,
    pub(crate) gmail_query: String,
    pub(crate) ordering: String,
    pub(crate) current_pointer: usize,
    pub(crate) items: Vec<QueueItem>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub(crate) struct QueueItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) index: Option<usize>,
    pub(crate) message_id: String,
    pub(crate) thread_id: String,
    pub(crate) internal_date: String,
    pub(crate) from: String,
    pub(crate) subject: String,
    #[serde(default)]
    pub(crate) snippet: String,
    #[serde(default = "default_status")]
    pub(crate) status: String,
    #[serde(default = "default_approval_state")]
    pub(crate) approval_state: String,
    #[serde(default = "default_research_state")]
    pub(crate) research_state: String,
    #[serde(default = "default_read_state")]
    pub(crate) read_state: String,
    #[serde(default)]
    pub(crate) dashboard_anchor: Option<String>,
    #[serde(default)]
    pub(crate) recommended_action: Option<String>,
    #[serde(default)]
    pub(crate) terminal_action: Option<String>,
    #[serde(default)]
    pub(crate) updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct QueueItemsPayload {
    pub(crate) items: Vec<Value>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdatesPayload {
    pub(crate) updates: Vec<QueueUpdate>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct QueueUpdate {
    pub(crate) message_id: String,
    pub(crate) fields: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct EventInput {
    pub(crate) event: String,
    #[serde(default)]
    pub(crate) message_id: Option<String>,
    #[serde(default)]
    pub(crate) data: Value,
    #[serde(default)]
    pub(crate) queue_update: BTreeMap<String, Value>,
    #[serde(default)]
    pub(crate) increments: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EventsPayload {
    pub(crate) events: Vec<EventInput>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SessionBatch {
    #[serde(default)]
    pub(crate) events: Vec<EventInput>,
    #[serde(default)]
    pub(crate) queue_updates: Vec<QueueUpdate>,
    #[serde(default)]
    pub(crate) stats_increments: Vec<String>,
    #[serde(default)]
    pub(crate) context_append: Vec<String>,
    #[serde(default)]
    pub(crate) dashboard_append: String,
    #[serde(default)]
    pub(crate) checkpoint_write: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct JournalEvent<'a> {
    pub(crate) ts: String,
    pub(crate) event: &'a str,
    pub(crate) message_id: Option<&'a str>,
    pub(crate) data: &'a Value,
}

pub(crate) fn default_status() -> String {
    "pending".into()
}

pub(crate) fn default_approval_state() -> String {
    "none".into()
}

pub(crate) fn default_research_state() -> String {
    "not_started".into()
}

pub(crate) fn default_read_state() -> String {
    "unknown".into()
}
