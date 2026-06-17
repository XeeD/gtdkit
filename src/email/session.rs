use std::{
    collections::BTreeMap,
    fs::File,
    path::{Path, PathBuf},
};

use chrono::{DateTime, Datelike, FixedOffset, Timelike};
use fs4::fs_std::FileExt;
use miette::{IntoDiagnostic, Result, bail, miette};
use serde_json::{Value, json};

use crate::{
    cli::SessionInitArgs,
    defaults::{LONG_TERM_MEMORY, base_stats, knowledge_base, newsletter_whitelist},
    fs_store::{
        ensure_file_json, ensure_file_text, expand_path, read_json, write_json, write_text,
    },
    time::{iso, now},
};

use super::model::{Manifest, Queue};

/// Create a restartable inbox-processing session and seed the durable files.
///
/// This is a durable mutation boundary: it creates the dated session directory,
/// initializes queue/stats/journal/checkpoint files, and creates missing shared
/// SOP config/memory files. Validation happens before directory creation where
/// possible so a bad session ID or active-session conflict does not leave a
/// partial session behind.
pub(crate) fn init(args: SessionInitArgs) -> Result<()> {
    let root = expand_path(&args.root)?;
    let now = now(&args.timezone)?;
    let session_id = match args.session_id {
        Some(id) => validate_session_id(&id)?,
        None => format!(
            "email-{:04}{:02}{:02}-{:02}{:02}",
            now.year(),
            now.month(),
            now.day(),
            now.hour(),
            now.minute()
        ),
    };
    let (id_year, id_month, id_day) =
        parse_session_id_date(&session_id).ok_or_else(|| miette!("invalid session ID"))?;
    if id_year != format!("{:04}", now.year())
        || id_month != format!("{:02}", now.month())
        || id_day != format!("{:02}", now.day())
    {
        bail!("session ID date must match the current local date");
    }

    let session_dir = root
        .join(format!("{:04}", now.year()))
        .join(format!("{:02}", now.month()))
        .join(format!("{:02}", now.day()))
        .join(&session_id);
    if session_dir.exists() {
        bail!(
            "Session directory already exists: {}",
            session_dir.display()
        );
    }
    if !args.allow_active_session
        && let Some(active) = find_active_session_for_date(&root, &now)?
    {
        bail!(
            "Active session exists: {} ({})\nResume or close it before starting a new default session, or pass --allow-active-session to intentionally create another session.",
            active.session_id,
            active.path.display()
        );
    }

    std::fs::create_dir_all(session_dir.parent().unwrap()).into_diagnostic()?;
    std::fs::create_dir(&session_dir)
        .into_diagnostic()
        .map_err(|err| err.wrap_err(format!("failed to create {}", session_dir.display())))?;
    let config_dir = root.join("config");
    let memory_dir = root.join("memory");
    std::fs::create_dir_all(&config_dir).into_diagnostic()?;
    std::fs::create_dir_all(&memory_dir).into_diagnostic()?;

    let whitelist_path = config_dir.join("newsletter-whitelist.json");
    let knowledge_base_path = config_dir.join("knowledge-base.json");
    let long_term_memory_path = memory_dir.join("long-term.md");
    ensure_file_json(&whitelist_path, newsletter_whitelist())?;
    ensure_file_json(&knowledge_base_path, knowledge_base())?;
    ensure_file_text(&long_term_memory_path, LONG_TERM_MEMORY)?;

    let manifest = Manifest {
        schema_version: 1,
        created_at: iso(&now),
        account: args.account,
        gmail_query: args.gmail_query.clone(),
        ordering: "newest_to_oldest".into(),
        session_dir: session_dir.display().to_string(),
        newsletter_whitelist: whitelist_path.display().to_string(),
        knowledge_base_config: knowledge_base_path.display().to_string(),
        long_term_memory: long_term_memory_path.display().to_string(),
        contract: BTreeMap::from([
            (
                "approval_required_before_external_state_change".into(),
                true,
            ),
            ("finish_only_after_fresh_mail_check_empty".into(), true),
            ("process_one_email_at_a_time".into(), true),
        ]),
    };
    let queue = Queue {
        schema_version: 1,
        created_at: iso(&now),
        gmail_query: args.gmail_query.clone(),
        ordering: "newest_to_oldest".into(),
        current_pointer: 0,
        items: vec![],
    };

    write_json(&session_dir.join("manifest.json"), &manifest)?;
    write_json(&session_dir.join("queue.json"), &queue)?;
    write_json(&session_dir.join("stats.json"), &base_stats())?;
    let event = json!({
        "ts": iso(&now),
        "event": "session_initialized",
        "message_id": null,
        "data": {"session_dir": session_dir.display().to_string(), "session_id": session_id, "gmail_query": args.gmail_query}
    });
    write_text(
        &session_dir.join("events.jsonl"),
        &(serde_json::to_string(&event).into_diagnostic()? + "\n"),
    )?;
    write_text(
        &session_dir.join("context.md"),
        "# Session Context\n\nRecord durable context gathered during this inbox processing session.\n",
    )?;
    write_text(
        &session_dir.join("dashboards.md"),
        "# Email Dashboards\n\nAppend one dashboard per processed email.\n",
    )?;
    write_text(
        &session_dir.join("checkpoint.md"),
        &format!(
            "# Checkpoint\n\nSession ID: `{}`\n\nSession: `{}`\n\nNext step: populate `queue.json` from Gmail query `in:inbox`, newest to oldest, then process the first pending item.\n",
            session_id,
            session_dir.display()
        ),
    )?;
    anstream::println!("{session_id}");
    Ok(())
}

#[derive(Debug)]
pub(crate) struct ActiveSession {
    pub(crate) session_id: String,
    pub(crate) path: PathBuf,
}

/// Accept only the short local session ID shape used by the SOP contract.
///
/// Full paths and legacy `session-*` names are rejected intentionally; callers
/// must pass `email-YYYYMMDD-HHMM` and let `resolve_session` map it under the
/// configured root.
pub(crate) fn validate_session_id(id: &str) -> Result<String> {
    if parse_session_id_date(id).is_none() {
        bail!("session ID must match email-YYYYMMDD-HHMM");
    }
    Ok(id.to_owned())
}

/// Parse the date components from a short session ID after basic calendar checks.
pub(crate) fn parse_session_id_date(id: &str) -> Option<(String, String, String)> {
    if id.len() != "email-YYYYMMDD-HHMM".len() {
        return None;
    }
    let bytes = id.as_bytes();
    if &bytes[0..6] != b"email-" || bytes[14] != b'-' {
        return None;
    }
    if !bytes[6..14].iter().all(u8::is_ascii_digit) || !bytes[15..19].iter().all(u8::is_ascii_digit)
    {
        return None;
    }
    let month: u32 = id[10..12].parse().ok()?;
    let day: u32 = id[12..14].parse().ok()?;
    let hour: u32 = id[15..17].parse().ok()?;
    let minute: u32 = id[17..19].parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 {
        return None;
    }
    Some((
        id[6..10].to_owned(),
        id[10..12].to_owned(),
        id[12..14].to_owned(),
    ))
}

/// Resolve a short session ID to its dated directory under the configured root.
///
/// Session IDs are intentionally short local identifiers. Commands resolve them
/// under the configured SOP root so agents do not persist brittle full paths in
/// journals, prompts, or queue metadata.
pub(crate) fn resolve_session(root: &Path, session_id: &str) -> Result<PathBuf> {
    validate_session_id(session_id)?;
    let (year, month, day) =
        parse_session_id_date(session_id).ok_or_else(|| miette!("invalid session ID"))?;
    let session_dir = expand_path(root)?
        .join(year)
        .join(month)
        .join(day)
        .join(session_id);
    if !session_dir.exists() {
        bail!(
            "Session ID does not exist under root: {} ({})",
            session_id,
            session_dir.display()
        );
    }
    Ok(session_dir)
}

/// Return the first incomplete session for the current local date, if any.
///
/// `session init` uses this to prevent accidental parallel same-day sessions.
/// The check is date-scoped by design: older unfinished sessions may be useful
/// history, but they should not block a new day by default.
pub(crate) fn find_active_session_for_date(
    root: &Path,
    now: &DateTime<FixedOffset>,
) -> Result<Option<ActiveSession>> {
    let day_dir = root
        .join(format!("{:04}", now.year()))
        .join(format!("{:02}", now.month()))
        .join(format!("{:02}", now.day()));
    if !day_dir.exists() {
        return Ok(None);
    }
    let mut entries = std::fs::read_dir(&day_dir)
        .into_diagnostic()
        .map_err(|err| err.wrap_err(format!("failed to read {}", day_dir.display())))?
        .collect::<std::io::Result<Vec<_>>>()
        .into_diagnostic()?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        if !path.is_dir() || !path.join("manifest.json").exists() {
            continue;
        }
        if session_is_active(&path)? {
            let session_id = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("<unknown>")
                .to_owned();
            return Ok(Some(ActiveSession { session_id, path }));
        }
    }
    Ok(None)
}

/// Decide whether a session still needs processing from journal and queue state.
///
/// A `session_completed` event is authoritative. Otherwise, an empty queue is
/// treated as active because the workflow may still need initial inbox metadata
/// or a fresh-mail check.
pub(crate) fn session_is_active(session_dir: &Path) -> Result<bool> {
    if event_log_contains(session_dir, "session_completed")? {
        return Ok(false);
    }
    let queue_path = session_dir.join("queue.json");
    if !queue_path.exists() {
        return Ok(true);
    }
    let queue: Queue = read_json(&queue_path)?;
    if queue.items.is_empty() {
        return Ok(true);
    }
    Ok(queue.items.iter().any(|item| {
        matches!(
            item.status.as_str(),
            "pending" | "in_progress" | "waiting_for_user" | "blocked"
        )
    }))
}

fn event_log_contains(session_dir: &Path, event_name: &str) -> Result<bool> {
    let events_path = session_dir.join("events.jsonl");
    if !events_path.exists() {
        return Ok(false);
    }
    let text = std::fs::read_to_string(&events_path)
        .into_diagnostic()
        .map_err(|err| err.wrap_err(format!("failed to read {}", events_path.display())))?;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let value: Value = serde_json::from_str(line)
            .into_diagnostic()
            .map_err(|err| err.wrap_err(format!("failed to parse {}", events_path.display())))?;
        if value.get("event").and_then(Value::as_str) == Some(event_name) {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) struct SessionLock {
    file: File,
}

impl SessionLock {
    /// Acquire the per-session advisory lock used by every durable write path.
    ///
    /// The lock file lives inside the session directory so unrelated sessions can
    /// progress independently while overlapping writers for the same session are
    /// serialized.
    pub(crate) fn acquire(session_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(session_dir).into_diagnostic()?;
        let file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(session_dir.join(".session.lock"))
            .into_diagnostic()?;
        file.lock_exclusive().into_diagnostic()?;
        Ok(Self { file })
    }
}

impl Drop for SessionLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        email::model::{
            Queue, QueueItem, default_approval_state, default_read_state, default_research_state,
        },
        fs_store::{write_json, write_text},
    };
    use chrono::TimeZone;
    use tempfile::TempDir;

    fn minimal_queue(items: Vec<QueueItem>) -> Queue {
        Queue {
            schema_version: 1,
            created_at: "2026-05-31T09:00:00-05:00".into(),
            gmail_query: "in:inbox".into(),
            ordering: "newest_to_oldest".into(),
            current_pointer: 0,
            items,
        }
    }

    fn queue_item(status: &str) -> QueueItem {
        QueueItem {
            index: Some(0),
            message_id: "mid-1".into(),
            thread_id: Some("thread-1".into()),
            internal_date: Some("2026-05-31T09:00:00-05:00".into()),
            from: Some("Sender <sender@example.com>".into()),
            subject: Some("Subject".into()),
            snippet: Some(String::new()),
            metadata_state: "complete".into(),
            labels: vec![],
            status: status.into(),
            approval_state: default_approval_state(),
            research_state: default_research_state(),
            read_state: default_read_state(),
            dashboard_anchor: None,
            recommended_action: None,
            terminal_action: None,
            updated_at: None,
        }
    }

    fn session_dir(root: &Path, session_id: &str) -> PathBuf {
        root.join(&session_id[6..10])
            .join(&session_id[10..12])
            .join(&session_id[12..14])
            .join(session_id)
    }

    fn write_minimal_session(root: &Path, session_id: &str, queue: Queue) -> PathBuf {
        let dir = session_dir(root, session_id);
        std::fs::create_dir_all(&dir).unwrap();
        write_text(&dir.join("manifest.json"), "{}\n").unwrap();
        write_json(&dir.join("queue.json"), &queue).unwrap();
        write_text(&dir.join("events.jsonl"), "").unwrap();
        dir
    }

    #[test]
    fn session_id_parser_accepts_exact_local_id_shape() {
        assert_eq!(
            parse_session_id_date("email-20260531-0914"),
            Some(("2026".into(), "05".into(), "31".into()))
        );
        assert!(validate_session_id("email-20260531-0914").is_ok());
    }

    #[test]
    fn session_id_parser_rejects_legacy_paths_and_bad_dates() {
        for id in [
            "session-20260531-091400",
            "/tmp/email-20260531-0914",
            "email-20260531-091400",
            "email-20261331-0914",
            "email-20260531-2460",
        ] {
            assert!(validate_session_id(id).is_err(), "{id}");
        }
    }

    #[test]
    fn resolve_session_maps_id_to_dated_root() {
        let tmp = TempDir::new().unwrap();
        let session_id = "email-20260531-0914";
        let expected = write_minimal_session(tmp.path(), session_id, minimal_queue(vec![]));

        assert_eq!(resolve_session(tmp.path(), session_id).unwrap(), expected);
        assert!(resolve_session(tmp.path(), "email-20260531-0915").is_err());
    }

    #[test]
    fn active_session_detection_uses_queue_and_completion_event() {
        let tmp = TempDir::new().unwrap();
        let active = write_minimal_session(
            tmp.path(),
            "email-20260531-0914",
            minimal_queue(vec![queue_item("pending")]),
        );
        assert!(session_is_active(&active).unwrap());

        write_text(
            &active.join("events.jsonl"),
            r#"{"event":"session_completed","ts":"2026-05-31T09:15:00-05:00","message_id":null,"data":{}}"#,
        )
        .unwrap();
        assert!(!session_is_active(&active).unwrap());

        let terminal = write_minimal_session(
            tmp.path(),
            "email-20260531-0916",
            minimal_queue(vec![queue_item("archived")]),
        );
        assert!(!session_is_active(&terminal).unwrap());
    }

    #[test]
    fn find_active_session_scans_current_local_day_only() {
        let tmp = TempDir::new().unwrap();
        let now = FixedOffset::west_opt(5 * 60 * 60)
            .unwrap()
            .with_ymd_and_hms(2026, 5, 31, 9, 30, 0)
            .unwrap();
        write_minimal_session(
            tmp.path(),
            "email-20260530-0914",
            minimal_queue(vec![queue_item("pending")]),
        );
        write_minimal_session(
            tmp.path(),
            "email-20260531-0914",
            minimal_queue(vec![queue_item("pending")]),
        );

        let active = find_active_session_for_date(tmp.path(), &now)
            .unwrap()
            .unwrap();
        assert_eq!(active.session_id, "email-20260531-0914");
    }
}
