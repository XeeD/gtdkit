use std::{collections::BTreeMap, io::Read};

use miette::{IntoDiagnostic, Result, bail};
use serde_json::{Value, json};

use crate::{
    cli::{
        ActionApproveArgs, ActionCompleteArgs, DashboardArgs, FreshCheckArgs, ResearchDigestArgs,
    },
    fs_store::{append_text, read_json, write_json, write_text},
    time::{iso, now},
};

use super::{
    journal::{append_events, increment_stats},
    model::{EventInput, Queue, QueueUpdate},
    queue::{apply_queue_updates, assert_queue_contains, clean_fields, validate_queue_updates},
    session::{SessionLock, resolve_session},
};

/// Record a returned research-agent digest without touching external services.
///
/// This command captures the read-only subagent result, sets queue research
/// state, and optionally stores the recommended action. It is intentionally
/// local-only: Gmail, browser, and OmniFocus mutations must be done by the main
/// agent after user approval.
pub(crate) fn research_digest(args: ResearchDigestArgs) -> Result<()> {
    let session_dir = resolve_session(&args.root, &args.session_id)?;
    let _lock = SessionLock::acquire(&session_dir)?;
    assert_queue_contains(&session_dir, &args.message_id)?;

    let mut fields = BTreeMap::from([("research_state".into(), Value::String(args.state))]);
    if !args.recommended_action.is_empty() {
        fields.insert(
            "recommended_action".into(),
            Value::String(args.recommended_action.clone()),
        );
    }

    let data = json!({
        "agent_id": empty_as_null(&args.agent_id),
        "agent_name": empty_as_null(&args.agent_name),
        "queue_index": args.queue_index,
        "recommended_action": empty_as_null(&args.recommended_action),
        "no_mutations_performed": args.no_mutations_performed,
        "digest": args.digest,
    });
    apply_locked_step(
        &session_dir,
        vec![
            EventInput {
                event: "subagent_digest_received".into(),
                message_id: Some(args.message_id.clone()),
                data,
                queue_update: BTreeMap::new(),
                increments: vec![],
            },
            EventInput {
                event: "research_buffered".into(),
                message_id: Some(args.message_id.clone()),
                data: json!({}),
                queue_update: BTreeMap::new(),
                increments: vec![],
            },
        ],
        vec![QueueUpdate {
            message_id: args.message_id,
            fields,
        }],
        vec![],
        None,
        None,
        &args.timezone,
    )
}

/// Record that a digest has become the user-facing dashboard step.
///
/// The dashboard body may come from stdin so agents can avoid creating
/// temporary JSON files for rich Markdown. The dashboard anchor is stored in
/// queue metadata; user-facing chat should render plain Markdown without XML
/// wrappers.
pub(crate) fn dashboard(args: DashboardArgs) -> Result<()> {
    if args.dashboard_stdin && !args.dashboard_text.is_empty() {
        bail!("use either --dashboard-stdin or --dashboard-text, not both");
    }
    let dashboard = if args.dashboard_stdin {
        let mut text = String::new();
        std::io::stdin()
            .read_to_string(&mut text)
            .into_diagnostic()?;
        text
    } else {
        args.dashboard_text
    };
    if dashboard.trim().is_empty() {
        bail!("dashboard text is required");
    }

    let session_dir = resolve_session(&args.root, &args.session_id)?;
    let _lock = SessionLock::acquire(&session_dir)?;
    assert_queue_contains(&session_dir, &args.message_id)?;

    let mut events = vec![];
    if args.email_started {
        events.push(EventInput {
            event: "email_started".into(),
            message_id: Some(args.message_id.clone()),
            data: json!({ "dashboard_anchor": args.dashboard_anchor }),
            queue_update: BTreeMap::new(),
            increments: vec![],
        });
    }
    events.extend([
        EventInput {
            event: "research_completed".into(),
            message_id: Some(args.message_id.clone()),
            data: json!({ "dashboard_anchor": args.dashboard_anchor }),
            queue_update: BTreeMap::new(),
            increments: vec![],
        },
        EventInput {
            event: "dashboard_rendered".into(),
            message_id: Some(args.message_id.clone()),
            data: json!({ "dashboard_anchor": args.dashboard_anchor }),
            queue_update: BTreeMap::new(),
            increments: vec![],
        },
        EventInput {
            event: "approval_requested".into(),
            message_id: Some(args.message_id.clone()),
            data: json!({
                "dashboard_anchor": args.dashboard_anchor,
                "approval_options": args.approval_options,
            }),
            queue_update: BTreeMap::new(),
            increments: vec![],
        },
    ]);

    let fields = clean_fields(BTreeMap::from([
        ("status".into(), Value::String("waiting_for_user".into())),
        ("approval_state".into(), Value::String("requested".into())),
        ("research_state".into(), Value::String("complete".into())),
        ("read_state".into(), Value::String(args.read_state.clone())),
        (
            "dashboard_anchor".into(),
            Value::String(args.dashboard_anchor.clone()),
        ),
        (
            "recommended_action".into(),
            Value::String(args.recommended_action),
        ),
    ]));
    apply_locked_step(
        &session_dir,
        events,
        vec![QueueUpdate {
            message_id: args.message_id,
            fields,
        }],
        vec![],
        Some(format!(
            "\n<a id=\"{}\"></a>\n\n{}\n",
            args.dashboard_anchor,
            dashboard.trim_end()
        )),
        Some(format!(
            "# Checkpoint\n\nCurrent dashboard: `{}`\n\nNext step: wait for user approval.\n",
            args.dashboard_anchor
        )),
        &args.timezone,
    )
}

/// Record explicit user approval for a pending action.
pub(crate) fn action_approve(args: ActionApproveArgs) -> Result<()> {
    let session_dir = resolve_session(&args.root, &args.session_id)?;
    let _lock = SessionLock::acquire(&session_dir)?;
    assert_queue_contains(&session_dir, &args.message_id)?;
    apply_locked_step(
        &session_dir,
        vec![EventInput {
            event: "action_approved".into(),
            message_id: Some(args.message_id.clone()),
            data: json!({
                "action": args.action,
                "user_reply": args.user_reply,
            }),
            queue_update: BTreeMap::new(),
            increments: vec![],
        }],
        vec![QueueUpdate {
            message_id: args.message_id,
            fields: BTreeMap::from([("approval_state".into(), Value::String("approved".into()))]),
        }],
        vec![],
        None,
        None,
        &args.timezone,
    )
}

/// Record completion of a terminal local workflow action after external proof.
///
/// `gtdkit` does not perform Gmail mutations. The caller supplies the Gmail
/// action name and verification text after the approved external operation
/// succeeds, and this command records the durable local transition.
pub(crate) fn action_complete(args: ActionCompleteArgs) -> Result<()> {
    let session_dir = resolve_session(&args.root, &args.session_id)?;
    let _lock = SessionLock::acquire(&session_dir)?;
    assert_queue_contains(&session_dir, &args.message_id)?;
    apply_locked_step(
        &session_dir,
        vec![EventInput {
            event: "action_completed".into(),
            message_id: Some(args.message_id.clone()),
            data: json!({
                "terminal_action": args.terminal_action,
                "gmail_action": empty_as_null(&args.gmail_action),
                "verification": args.verification,
            }),
            queue_update: BTreeMap::new(),
            increments: vec![],
        }],
        vec![QueueUpdate {
            message_id: args.message_id,
            fields: BTreeMap::from([
                ("status".into(), Value::String(args.terminal_action.clone())),
                ("terminal_action".into(), Value::String(args.terminal_action)),
            ]),
        }],
        args.stats,
        None,
        Some("# Checkpoint\n\nNext step: continue to the next pending email or run a fresh-mail check if the queue is empty.\n".into()),
        &args.timezone,
    )
}

/// Record the result of a fresh Gmail inbox check without querying Gmail itself.
pub(crate) fn fresh_check(args: FreshCheckArgs) -> Result<()> {
    if args.count != args.message_ids.len() {
        bail!("--count must match the number of --message-id values");
    }
    let session_dir = resolve_session(&args.root, &args.session_id)?;
    let _lock = SessionLock::acquire(&session_dir)?;
    apply_locked_step(
        &session_dir,
        vec![EventInput {
            event: "fresh_mail_check".into(),
            message_id: None,
            data: json!({
                "count": args.count,
                "message_ids": args.message_ids,
            }),
            queue_update: BTreeMap::new(),
            increments: vec![],
        }],
        vec![],
        vec!["fresh_mail_checks".into()],
        None,
        None,
        &args.timezone,
    )
}

/// Shared locked mutation helper for semantic workflow commands.
///
/// Callers perform command-specific validation first, then this function
/// validates queue updates, appends journal events, updates stats, applies queue
/// transitions, and appends/writes Markdown artifacts in one critical section.
fn apply_locked_step(
    session_dir: &std::path::Path,
    events: Vec<EventInput>,
    queue_updates: Vec<QueueUpdate>,
    stats: Vec<String>,
    dashboard_append: Option<String>,
    checkpoint_write: Option<String>,
    timezone: &str,
) -> Result<()> {
    validate_queue_updates(&queue_updates, "queue update")?;
    let now = now(timezone)?;
    append_events(session_dir, &events, &now)?;
    increment_stats(session_dir, &stats)?;
    if !queue_updates.is_empty() {
        let mut queue: Queue = read_json(&session_dir.join("queue.json"))?;
        apply_queue_updates(
            &mut queue,
            &queue_updates,
            &iso(&now),
            "Message ID not found in queue",
        )?;
        write_json(&session_dir.join("queue.json"), &queue)?;
    }
    if let Some(text) = dashboard_append {
        append_text(&session_dir.join("dashboards.md"), &text)?;
    }
    if let Some(text) = checkpoint_write {
        write_text(&session_dir.join("checkpoint.md"), &text)?;
    }
    Ok(())
}

fn empty_as_null(value: &str) -> Value {
    if value.is_empty() {
        Value::Null
    } else {
        Value::String(value.to_owned())
    }
}
