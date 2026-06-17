use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use assert_cmd::Command;
use assert_fs::{TempDir, prelude::*};
use chrono::{Datelike, Duration, Local, Timelike};
use predicates::prelude::*;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

fn bin() -> Command {
    Command::cargo_bin("gtdkit").unwrap()
}

fn base_stats() -> BTreeMap<String, i64> {
    [
        "archived",
        "deleted",
        "replied",
        "drafted",
        "sent",
        "omnifocus_tasks_created",
        "omnifocus_projects_created",
        "commitment_tasks_suggested",
        "waiting_for_external",
        "incubated",
        "marked_read",
        "knowledge_base_candidates",
        "references_filed",
        "unsubscribed",
        "newsletters_whitelisted",
        "links_opened",
        "web_searches",
        "gmail_threads_read",
        "gmail_history_searches",
        "browser_content_links_opened",
        "browser_state_changes_approved",
        "fresh_mail_checks",
        "memory_candidates_proposed",
        "memory_entries_added",
    ]
    .into_iter()
    .map(|key| (key.to_owned(), 0))
    .collect()
}

fn queue_item(message_id: &str, index: usize) -> Value {
    json!({
        "index": index,
        "message_id": message_id,
        "thread_id": format!("thread-{message_id}"),
        "internal_date": "2026-04-27T10:00:00-05:00",
        "from": "Sender <sender@example.com>",
        "subject": format!("Subject {message_id}"),
        "snippet": "",
        "metadata_state": "complete",
        "labels": [],
        "status": "pending",
        "approval_state": "none",
        "research_state": "not_started",
        "read_state": "unknown",
        "dashboard_anchor": null,
        "recommended_action": null,
        "terminal_action": null,
        "updated_at": null
    })
}

fn sparse_queue_item(message_id: &str, index: usize) -> Value {
    json!({
        "index": index,
        "message_id": message_id,
        "thread_id": null,
        "internal_date": null,
        "from": null,
        "subject": null,
        "snippet": null,
        "metadata_state": "sparse",
        "labels": [],
        "status": "pending",
        "approval_state": "none",
        "research_state": "not_started",
        "read_state": "unknown",
        "dashboard_anchor": null,
        "recommended_action": null,
        "terminal_action": null,
        "updated_at": null
    })
}

fn write_session(dir: &Path, items: Vec<Value>) {
    fs::create_dir_all(dir).unwrap();
    fs::write(
        dir.join("queue.json"),
        serde_json::to_string_pretty(&json!({
            "schema_version": 1,
            "created_at": "2026-04-27T10:00:00-05:00",
            "gmail_query": "in:inbox",
            "ordering": "newest_to_oldest",
            "current_pointer": 0,
            "items": items,
        }))
        .unwrap()
            + "\n",
    )
    .unwrap();
    fs::write(
        dir.join("stats.json"),
        serde_json::to_string_pretty(&base_stats()).unwrap() + "\n",
    )
    .unwrap();
    fs::write(dir.join("events.jsonl"), "").unwrap();
    fs::write(dir.join("context.md"), "").unwrap();
    fs::write(dir.join("dashboards.md"), "").unwrap();
    fs::write(dir.join("checkpoint.md"), "").unwrap();
}

fn session_path(root: &Path, session_id: &str) -> PathBuf {
    root.join(&session_id[6..10])
        .join(&session_id[10..12])
        .join(&session_id[12..14])
        .join(session_id)
}

fn write_session_for_id(root: &Path, session_id: &str, items: Vec<Value>) -> PathBuf {
    let dir = session_path(root, session_id);
    write_session(&dir, items);
    fs::write(
        dir.join("manifest.json"),
        serde_json::to_string_pretty(&json!({
            "schema_version": 1,
            "created_at": "2026-04-27T10:00:00-05:00",
            "account": "",
            "gmail_query": "in:inbox",
            "ordering": "newest_to_oldest",
            "session_dir": dir.display().to_string(),
            "newsletter_whitelist": root.join("config/newsletter-whitelist.json").display().to_string(),
            "knowledge_base_config": root.join("config/knowledge-base.json").display().to_string(),
            "long_term_memory": root.join("memory/long-term.md").display().to_string(),
            "contract": {}
        }))
        .unwrap()
            + "\n",
    )
    .unwrap();
    dir
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn local_session_id(offset_minutes: i64) -> String {
    let now = Local::now() + Duration::minutes(offset_minutes);
    format!(
        "email-{:04}{:02}{:02}-{:02}{:02}",
        now.year(),
        now.month(),
        now.day(),
        now.hour(),
        now.minute()
    )
}

fn different_local_session_id_same_day() -> String {
    let now = Local::now();
    let offset = if now.minute() == 59 { -1 } else { 1 };
    local_session_id(offset)
}

#[test]
fn init_session_creates_restartable_files() {
    let tmp = TempDir::new().unwrap();
    let output = bin()
        .args([
            "email",
            "session",
            "init",
            "--root",
            tmp.path().to_str().unwrap(),
            "--account",
            "user@example.com",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let session_id = std::str::from_utf8(&output).unwrap().trim();
    assert!(session_id.starts_with("email-"));
    let session_dir = session_path(tmp.path(), session_id);
    for name in [
        "manifest.json",
        "queue.json",
        "stats.json",
        "events.jsonl",
        "context.md",
        "dashboards.md",
        "checkpoint.md",
    ] {
        assert!(session_dir.join(name).exists(), "{name}");
    }
    assert_eq!(
        read_json(&session_dir.join("manifest.json"))["account"],
        "user@example.com"
    );
    assert_eq!(
        read_json(&session_dir.join("queue.json"))["items"],
        json!([])
    );
}

#[test]
fn init_refuses_second_active_session_for_current_local_date() {
    let tmp = TempDir::new().unwrap();
    bin()
        .args([
            "email",
            "session",
            "init",
            "--root",
            tmp.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let second_id = different_local_session_id_same_day();
    bin()
        .args([
            "email",
            "session",
            "init",
            &second_id,
            "--root",
            tmp.path().to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Active session exists"));

    bin()
        .args([
            "email",
            "session",
            "init",
            &second_id,
            "--root",
            tmp.path().to_str().unwrap(),
            "--allow-active-session",
        ])
        .assert()
        .success();
}

#[test]
fn queue_build_appends_replaces_and_rejects_duplicates_without_mutation() {
    let tmp = TempDir::new().unwrap();
    let session_id = "email-20260427-1000";
    let session = write_session_for_id(tmp.path(), session_id, vec![queue_item("mid-1", 0)]);

    let item = json!({
        "message_id": "mid-2",
        "thread_id": "thread-2",
        "internal_date": "2026-04-27T09:00:00-05:00",
        "from": "Other <other@example.com>",
        "subject": "Other"
    });
    bin()
        .args([
            "email",
            "queue",
            "build",
            session_id,
            "--root",
            tmp.path().to_str().unwrap(),
            "--item",
            &item.to_string(),
        ])
        .assert()
        .success();
    let queue = read_json(&session.join("queue.json"));
    assert_eq!(queue["items"][1]["message_id"], "mid-2");
    assert_eq!(queue["items"][1]["index"], 1);
    assert_eq!(queue["items"][1]["status"], "pending");

    bin()
        .args([
            "email",
            "queue",
            "build",
            session_id,
            "--root",
            tmp.path().to_str().unwrap(),
            "--item",
            &item.to_string(),
            "--item",
            &item.to_string(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("message ID already exists"));
    assert_eq!(
        read_json(&session.join("queue.json"))["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let replacement = json!({
        "message_id": "mid-9",
        "thread_id": "thread-9",
        "internal_date": "2026-04-27T09:00:00-05:00",
        "from": "Other <other@example.com>",
        "subject": "Replacement"
    });
    bin()
        .args([
            "email",
            "queue",
            "build",
            session_id,
            "--root",
            tmp.path().to_str().unwrap(),
            "--replace",
            "--item",
            &replacement.to_string(),
        ])
        .assert()
        .success();
    let queue = read_json(&session.join("queue.json"));
    assert_eq!(queue["current_pointer"], 0);
    assert_eq!(queue["items"][0]["message_id"], "mid-9");
    assert_eq!(queue["items"][0]["index"], 0);
}

#[test]
fn queue_build_accepts_sparse_metadata_and_preserves_input_order() {
    let tmp = TempDir::new().unwrap();
    let session_id = "email-20260427-1000";
    let session = write_session_for_id(tmp.path(), session_id, vec![]);

    let first = json!({"message_id": "mid-newest"});
    let second = json!({"message_id": "mid-middle", "thread_id": "thread-middle"});
    let third = json!({"message_id": "mid-oldest"});
    bin()
        .args([
            "email",
            "queue",
            "build",
            session_id,
            "--root",
            tmp.path().to_str().unwrap(),
            "--item",
            &first.to_string(),
            "--item",
            &second.to_string(),
            "--item",
            &third.to_string(),
            "--replace",
        ])
        .assert()
        .success();

    let queue = read_json(&session.join("queue.json"));
    assert_eq!(queue["items"][0]["message_id"], "mid-newest");
    assert_eq!(queue["items"][1]["message_id"], "mid-middle");
    assert_eq!(queue["items"][2]["message_id"], "mid-oldest");
    assert_eq!(queue["items"][0]["metadata_state"], "sparse");
    assert_eq!(queue["items"][1]["metadata_state"], "partial");
    assert_eq!(queue["items"][0]["internal_date"], Value::Null);

    let output = bin()
        .args([
            "email",
            "queue",
            "view",
            session_id,
            "--root",
            tmp.path().to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let payload: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(payload["queue_length"], 3);
    assert_eq!(payload["items"][1]["thread_id"], "thread-middle");
}

#[test]
fn queue_update_validates_before_mutation() {
    let tmp = TempDir::new().unwrap();
    let session_id = "email-20260427-1000";
    let session = write_session_for_id(tmp.path(), session_id, vec![queue_item("mid-1", 0)]);
    let bad = tmp.child("bad-updates.json");
    bad.write_str(
        &json!({"updates": [{"message_id": "mid-1", "fields": {"status": "deleted", "bogus": true}}]}).to_string(),
    )
    .unwrap();

    bin()
        .args([
            "email",
            "queue",
            "update",
            session_id,
            "--root",
            tmp.path().to_str().unwrap(),
            "--update-file",
            bad.path().to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsupported fields"));
    assert_eq!(
        read_json(&session.join("queue.json"))["items"][0]["status"],
        "pending"
    );
}

#[test]
fn queue_view_json_filters_pending() {
    let tmp = TempDir::new().unwrap();
    let session_id = "email-20260427-1000";
    write_session_for_id(
        tmp.path(),
        session_id,
        vec![queue_item("mid-1", 0), {
            let mut item = queue_item("mid-2", 1);
            item["status"] = json!("deleted");
            item
        }],
    );
    let output = bin()
        .args([
            "email",
            "queue",
            "view",
            session_id,
            "--root",
            tmp.path().to_str().unwrap(),
            "--status",
            "pending",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let payload: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(payload["filtered_count"], 1);
    assert_eq!(payload["items"][0]["message_id"], "mid-1");
}

#[test]
fn session_apply_updates_all_files() {
    let tmp = TempDir::new().unwrap();
    let session_id = "email-20260427-1000";
    let session = write_session_for_id(tmp.path(), session_id, vec![queue_item("mid-1", 0)]);
    bin()
        .args([
            "email",
            "session",
            "apply",
            session_id,
            "--root",
            tmp.path().to_str().unwrap(),
            "--event",
            r#"{"event":"dashboard_rendered","message_id":"mid-1","data":{"anchor":"email-0001"}}"#,
            "--queue-update",
            r#"{"message_id":"mid-1","fields":{"status":"waiting_for_user","approval_state":"requested","dashboard_anchor":"email-0001"}}"#,
            "--stat-increment",
            "gmail_threads_read",
            "--context-append",
            "- context line",
            "--dashboard-append",
            r#"<email_dashboard id="email-0001">x</email_dashboard>"#,
            "--checkpoint-write",
            "# Checkpoint\nwaiting",
        ])
        .assert()
        .success();
    assert!(
        fs::read_to_string(session.join("events.jsonl"))
            .unwrap()
            .contains("dashboard_rendered")
    );
    assert!(
        fs::read_to_string(session.join("context.md"))
            .unwrap()
            .contains("- context line")
    );
    assert!(
        fs::read_to_string(session.join("dashboards.md"))
            .unwrap()
            .contains("email-0001")
    );
    assert_eq!(
        fs::read_to_string(session.join("checkpoint.md")).unwrap(),
        "# Checkpoint\nwaiting"
    );
    assert_eq!(
        read_json(&session.join("stats.json"))["gmail_threads_read"],
        1
    );
    assert_eq!(
        read_json(&session.join("queue.json"))["items"][0]["dashboard_anchor"],
        "email-0001"
    );
}

#[test]
fn journal_event_and_batch_update_queue_and_stats() {
    let tmp = TempDir::new().unwrap();
    let session_id = "email-20260427-1000";
    let session = write_session_for_id(tmp.path(), session_id, vec![queue_item("mid-1", 0)]);

    bin()
        .args([
            "email",
            "journal",
            "event",
            session_id,
            "gmail_marked_read",
            "--root",
            tmp.path().to_str().unwrap(),
            "--message-id",
            "mid-1",
            "--data",
            r#"{"action":"remove UNREAD"}"#,
            "--set-read-state",
            "marked_read",
            "--increment",
            "marked_read",
        ])
        .assert()
        .success();
    assert_eq!(
        read_json(&session.join("queue.json"))["items"][0]["read_state"],
        "marked_read"
    );
    assert_eq!(read_json(&session.join("stats.json"))["marked_read"], 1);

    let batch = tmp.child("journal-batch.json");
    batch
        .write_str(
            &json!({"events": [
                {"event":"a","message_id":"mid-1","queue_update":{"status":"in_progress"}},
                {"event":"b","message_id":"mid-1","queue_update":{"research_state":"complete"},"increments":["gmail_threads_read"]}
            ]})
            .to_string(),
        )
        .unwrap();
    bin()
        .args([
            "email",
            "journal",
            "batch",
            session_id,
            "--root",
            tmp.path().to_str().unwrap(),
            "--batch-file",
            batch.path().to_str().unwrap(),
        ])
        .assert()
        .success();
    let item = &read_json(&session.join("queue.json"))["items"][0];
    assert_eq!(item["status"], "in_progress");
    assert_eq!(item["research_state"], "complete");
    assert_eq!(
        read_json(&session.join("stats.json"))["gmail_threads_read"],
        1
    );
}

#[test]
fn journal_event_accepts_event_flag() {
    let tmp = TempDir::new().unwrap();
    let session_id = "email-20260427-1000";
    let session = write_session_for_id(tmp.path(), session_id, vec![queue_item("mid-1", 0)]);

    bin()
        .args([
            "email",
            "journal",
            "event",
            session_id,
            "--root",
            tmp.path().to_str().unwrap(),
            "--event",
            "queue_build_started",
            "--data",
            r#"{"query":"in:inbox"}"#,
        ])
        .assert()
        .success();

    assert!(
        fs::read_to_string(session.join("events.jsonl"))
            .unwrap()
            .contains("queue_build_started")
    );
}

#[test]
fn queue_build_without_metadata_explains_it_does_not_query_gmail() {
    let tmp = TempDir::new().unwrap();
    let session_id = "email-20260427-1000";
    write_session_for_id(tmp.path(), session_id, vec![]);

    bin()
        .args([
            "email",
            "queue",
            "build",
            session_id,
            "--root",
            tmp.path().to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("query Gmail"));
}

#[test]
fn completions_zsh_emits_function() {
    bin()
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("#compdef gtdkit"));
}

#[test]
fn docs_cli_reference_matches_checked_in_file() {
    let output = bin()
        .args(["docs", "cli-reference", "--format", "markdown"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert_eq!(
        String::from_utf8(output).unwrap(),
        fs::read_to_string("docs/cli-reference.md").unwrap()
    );
}

#[test]
fn high_level_workflow_commands_update_session_without_batch_files() {
    let tmp = TempDir::new().unwrap();
    let session_id = "email-20260427-1000";
    let session = write_session_for_id(tmp.path(), session_id, vec![queue_item("mid-1", 0)]);

    bin()
        .args([
            "email",
            "research",
            "digest",
            session_id,
            "--root",
            tmp.path().to_str().unwrap(),
            "--message-id",
            "mid-1",
            "--queue-index",
            "0",
            "--agent-id",
            "agent-1",
            "--agent-name",
            "researcher",
            "--recommended-action",
            "archive",
            "--no-mutations-performed",
            "--digest",
            "No action needed.",
        ])
        .assert()
        .success();
    let item = &read_json(&session.join("queue.json"))["items"][0];
    assert_eq!(item["research_state"], "buffered");
    assert_eq!(item["recommended_action"], "archive");

    bin()
        .args([
            "email",
            "step",
            "dashboard",
            session_id,
            "--root",
            tmp.path().to_str().unwrap(),
            "--message-id",
            "mid-1",
            "--dashboard-anchor",
            "email-0001",
            "--recommended-action",
            "archive",
            "--approval-options",
            "archive,create-task",
            "--read-state",
            "read",
            "--dashboard-stdin",
            "--email-started",
        ])
        .write_stdin("**From:** Sender\n\nRecommended action: `archive`\n")
        .assert()
        .success();
    let item = &read_json(&session.join("queue.json"))["items"][0];
    assert_eq!(item["status"], "waiting_for_user");
    assert_eq!(item["approval_state"], "requested");
    assert_eq!(item["dashboard_anchor"], "email-0001");
    assert!(
        fs::read_to_string(session.join("dashboards.md"))
            .unwrap()
            .contains("Recommended action")
    );

    bin()
        .args([
            "email",
            "action",
            "approve",
            session_id,
            "--root",
            tmp.path().to_str().unwrap(),
            "--message-id",
            "mid-1",
            "--action",
            "archive",
            "--user-reply",
            "archive",
        ])
        .assert()
        .success();
    assert_eq!(
        read_json(&session.join("queue.json"))["items"][0]["approval_state"],
        "approved"
    );

    bin()
        .args([
            "email",
            "action",
            "complete",
            session_id,
            "--root",
            tmp.path().to_str().unwrap(),
            "--message-id",
            "mid-1",
            "--terminal-action",
            "archived",
            "--gmail-action",
            "archive",
            "--stat",
            "archived",
            "--verification",
            "in:inbox returned zero",
        ])
        .assert()
        .success();
    let item = &read_json(&session.join("queue.json"))["items"][0];
    assert_eq!(item["status"], "archived");
    assert_eq!(item["terminal_action"], "archived");
    assert_eq!(read_json(&session.join("stats.json"))["archived"], 1);
    assert!(
        fs::read_to_string(session.join("events.jsonl"))
            .unwrap()
            .contains("action_completed")
    );
}

#[test]
fn research_digest_backfills_sparse_queue_metadata_in_same_command() {
    let tmp = TempDir::new().unwrap();
    let session_id = "email-20260427-1000";
    let session = write_session_for_id(
        tmp.path(),
        session_id,
        vec![{
            let mut item = sparse_queue_item("mid-1", 0);
            item["updated_at"] = json!("old-timestamp");
            item
        }],
    );

    bin()
        .args([
            "email",
            "research",
            "digest",
            session_id,
            "--root",
            tmp.path().to_str().unwrap(),
            "--message-id",
            "mid-1",
            "--agent-id",
            "agent-1",
            "--recommended-action",
            "archive",
            "--no-mutations-performed",
            "--digest",
            "Read-only digest.",
            "--thread-id",
            "thread-1",
            "--internal-date",
            "2026-04-27T09:00:00-05:00",
            "--from",
            "Sender <sender@example.com>",
            "--subject",
            "Sparse fixed",
            "--snippet",
            "Preview text",
            "--label",
            "INBOX",
            "--label",
            "UNREAD",
        ])
        .assert()
        .success();

    let queue = read_json(&session.join("queue.json"));
    let item = &queue["items"][0];
    assert_eq!(item["thread_id"], "thread-1");
    assert_eq!(item["internal_date"], "2026-04-27T09:00:00-05:00");
    assert_eq!(item["from"], "Sender <sender@example.com>");
    assert_eq!(item["subject"], "Sparse fixed");
    assert_eq!(item["snippet"], "Preview text");
    assert_eq!(item["labels"], json!(["INBOX", "UNREAD"]));
    assert_eq!(item["metadata_state"], "complete");
    assert_ne!(item["updated_at"], "old-timestamp");

    let events = fs::read_to_string(session.join("events.jsonl")).unwrap();
    assert!(events.contains("subagent_digest_received"));
    assert!(events.contains("queue_metadata_enriched"));
}

#[test]
fn research_digest_ignores_empty_metadata_labels() {
    let tmp = TempDir::new().unwrap();
    let session_id = "email-20260427-1000";
    let session = write_session_for_id(tmp.path(), session_id, vec![sparse_queue_item("mid-1", 0)]);

    bin()
        .args([
            "email",
            "research",
            "digest",
            session_id,
            "--root",
            tmp.path().to_str().unwrap(),
            "--message-id",
            "mid-1",
            "--agent-id",
            "agent-1",
            "--no-mutations-performed",
            "--digest",
            "Read-only digest.",
            "--label",
            "",
            "--label",
            "   ",
        ])
        .assert()
        .success();

    let queue = read_json(&session.join("queue.json"));
    let item = &queue["items"][0];
    assert_eq!(item["labels"], json!([]));
    assert_eq!(item["metadata_state"], "sparse");

    let events = fs::read_to_string(session.join("events.jsonl")).unwrap();
    assert!(events.contains("subagent_digest_received"));
    assert!(!events.contains("queue_metadata_enriched"));
}

#[test]
fn high_level_workflow_validates_message_before_mutation() {
    let tmp = TempDir::new().unwrap();
    let session_id = "email-20260427-1000";
    let session = write_session_for_id(tmp.path(), session_id, vec![queue_item("mid-1", 0)]);
    let before_queue = fs::read_to_string(session.join("queue.json")).unwrap();
    let before_events = fs::read_to_string(session.join("events.jsonl")).unwrap();

    bin()
        .args([
            "email",
            "action",
            "complete",
            session_id,
            "--root",
            tmp.path().to_str().unwrap(),
            "--message-id",
            "missing-mid",
            "--terminal-action",
            "archived",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Message ID not found"));

    assert_eq!(
        fs::read_to_string(session.join("queue.json")).unwrap(),
        before_queue
    );
    assert_eq!(
        fs::read_to_string(session.join("events.jsonl")).unwrap(),
        before_events
    );
}

#[test]
fn fresh_check_records_count_and_validates_message_ids() {
    let tmp = TempDir::new().unwrap();
    let session_id = "email-20260427-1000";
    let session = write_session_for_id(tmp.path(), session_id, vec![]);

    bin()
        .args([
            "email",
            "fresh-check",
            session_id,
            "--root",
            tmp.path().to_str().unwrap(),
            "--count",
            "1",
            "--message-id",
            "mid-new",
        ])
        .assert()
        .success();

    assert_eq!(
        read_json(&session.join("stats.json"))["fresh_mail_checks"],
        1
    );
    assert!(
        fs::read_to_string(session.join("events.jsonl"))
            .unwrap()
            .contains("fresh_mail_check")
    );

    bin()
        .args([
            "email",
            "fresh-check",
            session_id,
            "--root",
            tmp.path().to_str().unwrap(),
            "--count",
            "2",
            "--message-id",
            "mid-new",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--count must match"));
}
