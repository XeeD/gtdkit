use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use comfy_table::{Table, presets::NOTHING};
use miette::{IntoDiagnostic, Result, bail, miette};
use serde_json::{Value, json};

use crate::{
    constants::QUEUE_FIELDS,
    fs_store::{expand_path, read_json, read_json_value, write_json},
    time::{iso, now},
};

use super::{
    model::{Queue, QueueItem, QueueItemsPayload, QueueUpdate, UpdatesPayload},
    session::{SessionLock, resolve_session},
};

/// Create or extend `queue.json` from already-collected Gmail metadata.
///
/// This command deliberately does not talk to Gmail. The caller must provide
/// metadata via `--item` or `--items-file`; `gtdkit` validates the item shape,
/// assigns stable queue indexes, and writes the queue under the session lock.
pub(crate) fn build(
    session_id: &str,
    root: &Path,
    items_file: Option<&Path>,
    item_args: &[String],
    replace: bool,
    timezone: &str,
) -> Result<()> {
    if items_file.is_some() && !item_args.is_empty() {
        bail!("use either --items-file or repeated --item arguments, not both");
    }
    let mut incoming = if let Some(path) = items_file {
        let raw = read_json_value(&expand_path(path)?)?;
        let values = match raw {
            Value::Array(items) => items,
            Value::Object(_) => {
                serde_json::from_value::<QueueItemsPayload>(raw)
                    .into_diagnostic()?
                    .items
            }
            _ => bail!("items file must contain a JSON array or an object with an items array"),
        };
        normalize_queue_items(values)?
    } else {
        if item_args.is_empty() {
            bail!(
                "provide either --items-file or at least one --item; queue build does not query Gmail, so collect inbox metadata first"
            );
        }
        normalize_queue_items(
            item_args
                .iter()
                .map(|raw| serde_json::from_str(raw).into_diagnostic())
                .collect::<Result<Vec<Value>>>()?,
        )?
    };
    if incoming.is_empty() {
        bail!("no queue items provided");
    }

    let session_dir = resolve_session(root, session_id)?;
    let _lock = SessionLock::acquire(&session_dir)?;
    let queue_path = session_dir.join("queue.json");
    let mut queue: Queue = read_json(&queue_path)?;
    let mut existing: BTreeSet<String> = if replace {
        queue.items.clear();
        queue.current_pointer = 0;
        BTreeSet::new()
    } else {
        queue
            .items
            .iter()
            .map(|item| item.message_id.clone())
            .collect()
    };
    let updated_at = iso(&now(timezone)?);
    let next_index = queue.items.len();
    for (offset, item) in incoming.iter_mut().enumerate() {
        if !existing.insert(item.message_id.clone()) {
            bail!("message ID already exists in queue: {}", item.message_id);
        }
        item.index = Some(next_index + offset);
        item.updated_at = Some(updated_at.clone());
    }
    queue.items.extend(incoming);
    write_json(&queue_path, &queue)
}

/// Print a locked, validated view of the queue for read-only inspection.
///
/// Agents should use this instead of reading `queue.json` directly so malformed
/// queue state fails through the same audited path as write commands.
pub(crate) fn view(
    session_id: &str,
    root: &Path,
    status: Option<&str>,
    limit: usize,
    json_output: bool,
) -> Result<()> {
    let session_dir = resolve_session(root, session_id)?;
    let _lock = SessionLock::acquire(&session_dir)?;
    let queue: Queue = read_json(&session_dir.join("queue.json"))?;
    let items: Vec<_> = queue
        .items
        .iter()
        .filter(|item| status.is_none_or(|status| item.status == status))
        .take(limit)
        .collect();
    if json_output {
        let payload = json!({
            "session_dir": session_dir.display().to_string(),
            "current_pointer": queue.current_pointer,
            "queue_length": queue.items.len(),
            "filtered_count": items.len(),
            "items": items,
        });
        anstream::println!(
            "{}",
            serde_json::to_string_pretty(&payload).into_diagnostic()?
        );
        return Ok(());
    }

    anstream::println!("Session: {}", session_dir.display());
    anstream::println!("Current pointer: {}", queue.current_pointer);
    anstream::println!("Queue length: {}", queue.items.len());
    if let Some(status) = status {
        anstream::println!("Filter: status={status}");
    }
    anstream::println!("Showing: {} item(s)", items.len());
    let mut table = Table::new();
    table.load_preset(NOTHING);
    for item in items {
        table.add_row([
            item.index
                .map(|value| value.to_string())
                .unwrap_or_default(),
            item.status.clone(),
            item.message_id.clone(),
            item.from.clone(),
            item.subject.clone(),
        ]);
    }
    if !table.is_empty() {
        anstream::println!("{table}");
    }
    Ok(())
}

/// Apply queue-only field updates from a JSON payload.
///
/// All update field names are validated before the session lock is acquired and
/// before `queue.json` is rewritten, which protects against partial mutation for
/// unsupported queue schema changes.
pub(crate) fn update(
    session_id: &str,
    root: &Path,
    update_file: &Path,
    timezone: &str,
) -> Result<()> {
    let raw = read_json_value(&expand_path(update_file)?)?;
    let updates = match raw {
        Value::Array(_) => serde_json::from_value::<Vec<QueueUpdate>>(raw).into_diagnostic()?,
        Value::Object(_) => {
            serde_json::from_value::<UpdatesPayload>(raw)
                .into_diagnostic()?
                .updates
        }
        _ => bail!("update file must contain a JSON array or an object with an updates array"),
    };
    validate_queue_updates(&updates, "update")?;

    let session_dir = resolve_session(root, session_id)?;
    let _lock = SessionLock::acquire(&session_dir)?;
    let queue_path = session_dir.join("queue.json");
    let mut queue: Queue = read_json(&queue_path)?;
    let updated_at = iso(&now(timezone)?);
    apply_queue_updates(
        &mut queue,
        &updates,
        &updated_at,
        "Message ID not found in queue",
    )?;
    write_json(&queue_path, &queue)
}

/// Apply validated queue field updates to in-memory queue state.
///
/// This is intentionally pure with respect to the filesystem; callers own
/// locking and writing. Sharing this function keeps low-level and workflow
/// commands aligned on the same status/field invariants.
pub(crate) fn apply_queue_updates(
    queue: &mut Queue,
    updates: &[QueueUpdate],
    updated_at: &str,
    missing_message: &str,
) -> Result<()> {
    for update in updates {
        validate_fields(&update.fields, "unsupported queue fields")?;
        let clean = clean_fields(update.fields.clone());
        let item = queue
            .items
            .iter_mut()
            .find(|item| item.message_id == update.message_id)
            .ok_or_else(|| miette!("{missing_message}: {}", update.message_id))?;
        apply_fields(item, clean)?;
        item.updated_at = Some(updated_at.into());
    }
    Ok(())
}

/// Apply a small set of string-like workflow fields to one queue item.
pub(crate) fn apply_fields(item: &mut QueueItem, fields: BTreeMap<String, Value>) -> Result<()> {
    for (key, value) in fields {
        let text_value = match value {
            Value::String(value) => value,
            other => other.to_string(),
        };
        match key.as_str() {
            "status" => item.status = text_value,
            "approval_state" => item.approval_state = text_value,
            "research_state" => item.research_state = text_value,
            "read_state" => item.read_state = text_value,
            "recommended_action" => item.recommended_action = Some(text_value),
            "terminal_action" => item.terminal_action = Some(text_value),
            "dashboard_anchor" => item.dashboard_anchor = Some(text_value),
            _ => bail!("unsupported queue fields: {key}"),
        }
    }
    Ok(())
}

/// Validate a set of queue updates without mutating session files.
pub(crate) fn validate_queue_updates(updates: &[QueueUpdate], label: &str) -> Result<()> {
    for (index, update) in updates.iter().enumerate() {
        if update.message_id.is_empty() {
            bail!("{label} {index} missing message_id");
        }
        validate_fields(
            &update.fields,
            &format!("{label} {index} has unsupported fields"),
        )?;
    }
    Ok(())
}

/// Enforce the queue field allow-list shared by all write paths.
pub(crate) fn validate_fields(fields: &BTreeMap<String, Value>, message: &str) -> Result<()> {
    let allowed: BTreeSet<_> = QUEUE_FIELDS.iter().copied().collect();
    let unknown: Vec<_> = fields
        .keys()
        .filter(|key| !allowed.contains(key.as_str()))
        .cloned()
        .collect();
    if !unknown.is_empty() {
        bail!("{message}: {}", unknown.join(", "));
    }
    Ok(())
}

/// Drop empty optional updates so direct CLI flags can omit unchanged fields.
pub(crate) fn clean_fields(fields: BTreeMap<String, Value>) -> BTreeMap<String, Value> {
    fields
        .into_iter()
        .filter(|(_, value)| match value {
            Value::Null => false,
            Value::String(value) => !value.is_empty(),
            _ => true,
        })
        .collect()
}

/// Guard a message-scoped mutation by proving the message is in the queue.
pub(crate) fn assert_queue_contains(session_dir: &Path, message_id: &str) -> Result<()> {
    let queue: Queue = read_json(&session_dir.join("queue.json"))?;
    if queue.items.iter().any(|item| item.message_id == message_id) {
        Ok(())
    } else {
        bail!("Message ID not found in queue: {message_id}")
    }
}

/// Normalize raw queue item JSON into typed items and strip caller indexes.
///
/// Queue metadata must stay body-agnostic: this rejects unsupported fields so
/// body-derived classifications or recommendations cannot be smuggled into the
/// queue before research/dashboard steps are journaled.
pub(crate) fn normalize_queue_items(values: Vec<Value>) -> Result<Vec<QueueItem>> {
    let mut items = vec![];
    for value in values {
        let object = value
            .as_object()
            .ok_or_else(|| miette!("queue item must be a JSON object"))?;
        for required in [
            "message_id",
            "thread_id",
            "internal_date",
            "from",
            "subject",
        ] {
            if !object.contains_key(required) {
                bail!("queue item missing required fields: {required}");
            }
        }
        let allowed: BTreeSet<_> = [
            "index",
            "message_id",
            "thread_id",
            "internal_date",
            "from",
            "subject",
            "snippet",
            "status",
            "approval_state",
            "research_state",
            "read_state",
            "dashboard_anchor",
            "recommended_action",
            "terminal_action",
            "updated_at",
        ]
        .into_iter()
        .collect();
        let unknown: Vec<_> = object
            .keys()
            .filter(|key| !allowed.contains(key.as_str()))
            .cloned()
            .collect();
        if !unknown.is_empty() {
            bail!("queue item has unsupported fields: {}", unknown.join(", "));
        }
        let mut item: QueueItem = serde_json::from_value(value).into_diagnostic()?;
        item.index = None;
        item.updated_at = None;
        items.push(item);
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;

    #[test]
    fn queue_field_validation_rejects_unknown_fields() {
        let fields = BTreeMap::from([
            ("status".to_string(), json!("archived")),
            ("bogus".to_string(), json!(true)),
        ]);

        assert!(validate_fields(&fields, "bad fields").is_err());
    }

    #[test]
    fn normalize_queue_items_rejects_body_derived_extra_fields() {
        let item = json!({
            "message_id": "mid-1",
            "thread_id": "thread-1",
            "internal_date": "2026-05-31T09:00:00-05:00",
            "from": "Sender <sender@example.com>",
            "subject": "Subject",
            "classification": "archive"
        });

        assert!(normalize_queue_items(vec![item]).is_err());
    }
}
