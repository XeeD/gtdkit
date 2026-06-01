use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use chrono::{DateTime, FixedOffset};
use miette::{IntoDiagnostic, Result, bail, miette};
use serde_json::{Value, json};

use crate::{
    cli::JournalEventArgs,
    fs_store::{expand_path, read_json, read_json_value, write_json},
    time::{iso, now},
};

use super::{
    model::{EventInput, EventsPayload, JournalEvent, Queue, QueueUpdate},
    queue::{apply_queue_updates, assert_queue_contains, clean_fields, validate_fields},
    session::{SessionLock, resolve_session},
};

/// Resolve the event name from the positional or `--event` spelling.
///
/// Both spellings exist for CLI ergonomics, but accepting conflicting values
/// would make the append-only journal ambiguous, so conflicts fail before any
/// session state is touched.
pub(crate) fn choose_event(positional: Option<String>, flag: Option<String>) -> Result<String> {
    match (positional, flag) {
        (Some(positional), Some(flag)) if positional != flag => {
            bail!("provide the event either positionally or with --event, not both")
        }
        (Some(event), _) | (_, Some(event)) => Ok(event),
        (None, None) => bail!("missing event"),
    }
}

/// Append one journal event and optional adjacent queue/stat updates.
///
/// This is the narrow single-step write path. It validates message-scoped queue
/// updates before appending the journal event so callers do not record an event
/// that claims a queue transition failed validation.
pub(crate) fn event(args: JournalEventArgs) -> Result<()> {
    let data = if args.data.is_empty() {
        json!({})
    } else {
        serde_json::from_str(&args.data).into_diagnostic()?
    };
    let queue_fields = clean_fields(BTreeMap::from([
        ("status".into(), Value::String(args.set_status)),
        (
            "approval_state".into(),
            Value::String(args.set_approval_state),
        ),
        (
            "research_state".into(),
            Value::String(args.set_research_state),
        ),
        ("read_state".into(), Value::String(args.set_read_state)),
        (
            "recommended_action".into(),
            Value::String(args.set_recommended_action),
        ),
        (
            "terminal_action".into(),
            Value::String(args.set_terminal_action),
        ),
        (
            "dashboard_anchor".into(),
            Value::String(args.set_dashboard_anchor),
        ),
    ]));
    if !queue_fields.is_empty() && args.message_id.is_none() {
        bail!("queue field updates require --message-id");
    }

    let session_dir = resolve_session(&args.root, &args.session_id)?;
    let _lock = SessionLock::acquire(&session_dir)?;
    let now = now(&args.timezone)?;
    if !queue_fields.is_empty() {
        assert_queue_contains(&session_dir, args.message_id.as_deref().unwrap())?;
    }
    append_events(
        &session_dir,
        &[EventInput {
            event: choose_event(args.event, args.event_name)?,
            message_id: args.message_id.clone(),
            data,
            queue_update: BTreeMap::new(),
            increments: vec![],
        }],
        &now,
    )?;
    increment_stats(&session_dir, &args.increments)?;
    if !queue_fields.is_empty() {
        let update = QueueUpdate {
            message_id: args.message_id.unwrap(),
            fields: queue_fields,
        };
        let mut queue: Queue = read_json(&session_dir.join("queue.json"))?;
        apply_queue_updates(
            &mut queue,
            &[update],
            &iso(&now),
            "Message ID not found in queue",
        )?;
        write_json(&session_dir.join("queue.json"), &queue)?;
    }
    Ok(())
}

/// Append a file-backed batch of journal events for compatibility workflows.
///
/// Higher-level workflow commands should be preferred for normal inbox
/// processing; this remains for prebuilt payloads and migration compatibility.
pub(crate) fn batch(
    session_id: &str,
    root: &Path,
    batch_file: &Path,
    timezone: &str,
) -> Result<()> {
    let raw = read_json_value(&expand_path(batch_file)?)?;
    let operations = match raw {
        Value::Array(_) => serde_json::from_value::<Vec<EventInput>>(raw).into_diagnostic()?,
        Value::Object(_) => {
            serde_json::from_value::<EventsPayload>(raw)
                .into_diagnostic()?
                .events
        }
        _ => bail!("batch file must contain valid events: events must be an array"),
    };
    validate_events(&operations)?;

    let session_dir = resolve_session(root, session_id)?;
    let _lock = SessionLock::acquire(&session_dir)?;
    append_event_operations(&session_dir, &operations, timezone)
}

/// Validate, merge, and apply event-driven queue/stat side effects.
///
/// The merge step lets multiple events for the same message produce one final
/// queue update while preserving every journal event in append order.
pub(crate) fn append_event_operations(
    session_dir: &Path,
    operations: &[EventInput],
    timezone: &str,
) -> Result<()> {
    let mut merged_updates: BTreeMap<String, BTreeMap<String, Value>> = BTreeMap::new();
    let mut increments = vec![];
    let queue: Queue = read_json(&session_dir.join("queue.json"))?;
    let message_ids: BTreeSet<_> = queue
        .items
        .iter()
        .map(|item| item.message_id.as_str())
        .collect();

    for (index, op) in operations.iter().enumerate() {
        let fields = clean_fields(op.queue_update.clone());
        validate_fields(&fields, "unsupported queue_update fields")?;
        if !fields.is_empty() {
            let message_id = op
                .message_id
                .as_deref()
                .ok_or_else(|| miette!("batch event {index} queue_update requires message_id"))?;
            if !message_ids.contains(message_id) {
                bail!("batch event {index} message ID not found in queue: {message_id}");
            }
            merged_updates
                .entry(message_id.into())
                .or_default()
                .extend(fields);
        }
        increments.extend(op.increments.clone());
    }
    let now = now(timezone)?;
    append_events(session_dir, operations, &now)?;
    increment_stats(session_dir, &increments)?;
    if !merged_updates.is_empty() {
        let mut queue: Queue = read_json(&session_dir.join("queue.json"))?;
        let updates: Vec<_> = merged_updates
            .into_iter()
            .map(|(message_id, fields)| QueueUpdate { message_id, fields })
            .collect();
        apply_queue_updates(
            &mut queue,
            &updates,
            &iso(&now),
            "Message ID not found in queue",
        )?;
        write_json(&session_dir.join("queue.json"), &queue)?;
    }
    Ok(())
}

/// Append events to `events.jsonl` with stable, monotonic timestamps.
///
/// Offsetting same-command events by microseconds preserves command order while
/// keeping each line independently parseable for later resume/replay logic.
pub(crate) fn append_events(
    session_dir: &Path,
    events: &[EventInput],
    now: &DateTime<FixedOffset>,
) -> Result<()> {
    if events.is_empty() {
        return Ok(());
    }
    let mut lines = String::new();
    for (offset, event) in events.iter().enumerate() {
        let ts = iso(&(*now + chrono::Duration::microseconds(offset as i64)));
        let line = JournalEvent {
            ts,
            event: &event.event,
            message_id: event.message_id.as_deref(),
            data: &event.data,
        };
        lines.push_str(&serde_json::to_string(&line).into_diagnostic()?);
        lines.push('\n');
    }
    crate::fs_store::append_text(&session_dir.join("events.jsonl"), &lines)
}

/// Increment named counters in `stats.json`, creating missing counters as zero.
pub(crate) fn increment_stats(session_dir: &Path, keys: &[String]) -> Result<()> {
    if keys.is_empty() {
        return Ok(());
    }
    let path = session_dir.join("stats.json");
    let mut stats: BTreeMap<String, i64> = read_json(&path)?;
    for key in keys {
        *stats.entry(key.clone()).or_insert(0) += 1;
    }
    write_json(&path, &stats)
}

/// Validate event records without touching durable session files.
pub(crate) fn validate_events(events: &[EventInput]) -> Result<()> {
    for (index, event) in events.iter().enumerate() {
        if event.event.is_empty() {
            bail!("event {index} missing event");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_validation_rejects_empty_names() {
        assert!(
            validate_events(&[EventInput {
                event: String::new(),
                message_id: None,
                data: json!({}),
                queue_update: BTreeMap::new(),
                increments: vec![],
            }])
            .is_err()
        );
    }
}
