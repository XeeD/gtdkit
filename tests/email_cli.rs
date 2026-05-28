use std::{collections::BTreeMap, fs, path::Path};

use assert_cmd::Command;
use assert_fs::{TempDir, prelude::*};
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

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
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
            "--session-id",
            "session-test",
            "--account",
            "user@example.com",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let session_dir = Path::new(std::str::from_utf8(&output).unwrap().trim());
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
fn queue_build_appends_replaces_and_rejects_duplicates_without_mutation() {
    let tmp = TempDir::new().unwrap();
    let session = tmp.child("session");
    write_session(session.path(), vec![queue_item("mid-1", 0)]);

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
            session.path().to_str().unwrap(),
            "--item",
            &item.to_string(),
        ])
        .assert()
        .success();
    let queue = read_json(&session.path().join("queue.json"));
    assert_eq!(queue["items"][1]["message_id"], "mid-2");
    assert_eq!(queue["items"][1]["index"], 1);
    assert_eq!(queue["items"][1]["status"], "pending");

    bin()
        .args([
            "email",
            "queue",
            "build",
            session.path().to_str().unwrap(),
            "--item",
            &item.to_string(),
            "--item",
            &item.to_string(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("message ID already exists"));
    assert_eq!(
        read_json(&session.path().join("queue.json"))["items"]
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
            session.path().to_str().unwrap(),
            "--replace",
            "--item",
            &replacement.to_string(),
        ])
        .assert()
        .success();
    let queue = read_json(&session.path().join("queue.json"));
    assert_eq!(queue["current_pointer"], 0);
    assert_eq!(queue["items"][0]["message_id"], "mid-9");
    assert_eq!(queue["items"][0]["index"], 0);
}

#[test]
fn queue_update_validates_before_mutation() {
    let tmp = TempDir::new().unwrap();
    let session = tmp.child("session");
    write_session(session.path(), vec![queue_item("mid-1", 0)]);
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
            session.path().to_str().unwrap(),
            "--update-file",
            bad.path().to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsupported fields"));
    assert_eq!(
        read_json(&session.path().join("queue.json"))["items"][0]["status"],
        "pending"
    );
}

#[test]
fn queue_view_json_filters_pending() {
    let tmp = TempDir::new().unwrap();
    let session = tmp.child("session");
    write_session(
        session.path(),
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
            session.path().to_str().unwrap(),
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
    let session = tmp.child("session");
    write_session(session.path(), vec![queue_item("mid-1", 0)]);
    bin()
        .args([
            "email",
            "session",
            "apply",
            session.path().to_str().unwrap(),
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
        fs::read_to_string(session.path().join("events.jsonl"))
            .unwrap()
            .contains("dashboard_rendered")
    );
    assert!(
        fs::read_to_string(session.path().join("context.md"))
            .unwrap()
            .contains("- context line")
    );
    assert!(
        fs::read_to_string(session.path().join("dashboards.md"))
            .unwrap()
            .contains("email-0001")
    );
    assert_eq!(
        fs::read_to_string(session.path().join("checkpoint.md")).unwrap(),
        "# Checkpoint\nwaiting"
    );
    assert_eq!(
        read_json(&session.path().join("stats.json"))["gmail_threads_read"],
        1
    );
    assert_eq!(
        read_json(&session.path().join("queue.json"))["items"][0]["dashboard_anchor"],
        "email-0001"
    );
}

#[test]
fn journal_event_and_batch_update_queue_and_stats() {
    let tmp = TempDir::new().unwrap();
    let session = tmp.child("session");
    write_session(session.path(), vec![queue_item("mid-1", 0)]);

    bin()
        .args([
            "email",
            "journal",
            "event",
            session.path().to_str().unwrap(),
            "gmail_marked_read",
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
        read_json(&session.path().join("queue.json"))["items"][0]["read_state"],
        "marked_read"
    );
    assert_eq!(
        read_json(&session.path().join("stats.json"))["marked_read"],
        1
    );

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
            session.path().to_str().unwrap(),
            "--batch-file",
            batch.path().to_str().unwrap(),
        ])
        .assert()
        .success();
    let item = &read_json(&session.path().join("queue.json"))["items"][0];
    assert_eq!(item["status"], "in_progress");
    assert_eq!(item["research_state"], "complete");
    assert_eq!(
        read_json(&session.path().join("stats.json"))["gmail_threads_read"],
        1
    );
}

#[test]
fn completions_zsh_emits_function() {
    bin()
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("#compdef gtdkit"));
}
