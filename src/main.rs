use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use anstream::{eprintln, println};
use camino::Utf8PathBuf;
use chrono::{DateTime, Datelike, FixedOffset, Local, SecondsFormat};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use comfy_table::{Table, presets::NOTHING};
use fs4::fs_std::FileExt;
use miette::{IntoDiagnostic, Result, bail, miette};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tempfile::NamedTempFile;

const DEFAULT_ROOT: &str = "/Users/xeed/Documents/SOPs/email-inbox-processing";
const DEFAULT_GMAIL_QUERY: &str = "in:inbox";
const DEFAULT_TZ: &str = "America/Chicago";
const QUEUE_FIELDS: &[&str] = &[
    "status",
    "approval_state",
    "research_state",
    "read_state",
    "recommended_action",
    "terminal_action",
    "dashboard_anchor",
];

#[derive(Debug, Parser)]
#[command(name = "gtdkit", version, about = "GTD workflow utilities")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
#[allow(clippy::large_enum_variant)]
enum Commands {
    Email {
        #[command(subcommand)]
        command: EmailCommands,
    },
    Completions {
        shell: CompletionShell,
    },
}

#[derive(Clone, Debug, ValueEnum)]
enum CompletionShell {
    Zsh,
}

#[derive(Debug, Subcommand)]
enum EmailCommands {
    Session {
        #[command(subcommand)]
        command: SessionCommands,
    },
    Queue {
        #[command(subcommand)]
        command: QueueCommands,
    },
    Journal {
        #[command(subcommand)]
        command: JournalCommands,
    },
}

#[derive(Debug, Subcommand)]
enum SessionCommands {
    Init {
        #[arg(long, default_value = DEFAULT_ROOT)]
        root: PathBuf,
        #[arg(long, default_value = DEFAULT_GMAIL_QUERY)]
        gmail_query: String,
        #[arg(long, default_value = "")]
        account: String,
        #[arg(long, default_value = DEFAULT_TZ)]
        timezone: String,
        #[arg(long, default_value = "")]
        session_id: String,
    },
    Apply {
        session_dir: PathBuf,
        #[arg(long)]
        batch_file: Option<PathBuf>,
        #[arg(long = "event")]
        events: Vec<String>,
        #[arg(long = "queue-update")]
        queue_updates: Vec<String>,
        #[arg(long = "stat-increment")]
        stat_increments: Vec<String>,
        #[arg(long = "context-append", allow_hyphen_values = true)]
        context_append: Vec<String>,
        #[arg(long, default_value = "", allow_hyphen_values = true)]
        dashboard_append: String,
        #[arg(long, default_value = "", allow_hyphen_values = true)]
        checkpoint_write: String,
        #[arg(long, default_value = DEFAULT_TZ)]
        timezone: String,
    },
}

#[derive(Debug, Subcommand)]
enum QueueCommands {
    Build {
        session_dir: PathBuf,
        #[arg(long)]
        items_file: Option<PathBuf>,
        #[arg(long = "item")]
        items: Vec<String>,
        #[arg(long)]
        replace: bool,
        #[arg(long, default_value = DEFAULT_TZ)]
        timezone: String,
    },
    View {
        session_dir: PathBuf,
        #[arg(long)]
        status: Option<String>,
        #[arg(long, default_value_t = 10)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    Update {
        session_dir: PathBuf,
        #[arg(long)]
        update_file: PathBuf,
        #[arg(long, default_value = DEFAULT_TZ)]
        timezone: String,
    },
}

#[derive(Debug, Subcommand)]
#[allow(clippy::large_enum_variant)]
enum JournalCommands {
    Event {
        #[arg(value_name = "SESSION_DIR", required_unless_present = "session")]
        session_dir: Option<PathBuf>,
        #[arg(value_name = "EVENT", required_unless_present = "event_name")]
        event: Option<String>,
        #[arg(long = "session", value_name = "SESSION_DIR")]
        session: Option<PathBuf>,
        #[arg(long = "event", value_name = "EVENT")]
        event_name: Option<String>,
        #[arg(long)]
        message_id: Option<String>,
        #[arg(long, default_value = "", allow_hyphen_values = true)]
        data: String,
        #[arg(long = "increment")]
        increments: Vec<String>,
        #[arg(long, default_value = "")]
        set_status: String,
        #[arg(long, default_value = "")]
        set_approval_state: String,
        #[arg(long, default_value = "")]
        set_research_state: String,
        #[arg(long, default_value = "")]
        set_read_state: String,
        #[arg(long, default_value = "", allow_hyphen_values = true)]
        set_recommended_action: String,
        #[arg(long, default_value = "", allow_hyphen_values = true)]
        set_terminal_action: String,
        #[arg(long, default_value = "")]
        set_dashboard_anchor: String,
        #[arg(long, default_value = DEFAULT_TZ)]
        timezone: String,
    },
    Batch {
        session_dir: PathBuf,
        #[arg(long)]
        batch_file: PathBuf,
        #[arg(long, default_value = DEFAULT_TZ)]
        timezone: String,
    },
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct Manifest {
    schema_version: u8,
    created_at: String,
    account: String,
    gmail_query: String,
    ordering: String,
    session_dir: String,
    newsletter_whitelist: String,
    knowledge_base_config: String,
    long_term_memory: String,
    contract: BTreeMap<String, bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
struct Queue {
    schema_version: u8,
    created_at: String,
    gmail_query: String,
    ordering: String,
    current_pointer: usize,
    items: Vec<QueueItem>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
struct QueueItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    index: Option<usize>,
    message_id: String,
    thread_id: String,
    internal_date: String,
    from: String,
    subject: String,
    #[serde(default)]
    snippet: String,
    #[serde(default = "default_status")]
    status: String,
    #[serde(default = "default_approval_state")]
    approval_state: String,
    #[serde(default = "default_research_state")]
    research_state: String,
    #[serde(default = "default_read_state")]
    read_state: String,
    #[serde(default)]
    dashboard_anchor: Option<String>,
    #[serde(default)]
    recommended_action: Option<String>,
    #[serde(default)]
    terminal_action: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct QueueItemsPayload {
    items: Vec<Value>,
}

#[derive(Debug, Deserialize)]
struct UpdatesPayload {
    updates: Vec<QueueUpdate>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct QueueUpdate {
    message_id: String,
    fields: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct EventInput {
    event: String,
    #[serde(default)]
    message_id: Option<String>,
    #[serde(default)]
    data: Value,
    #[serde(default)]
    queue_update: BTreeMap<String, Value>,
    #[serde(default)]
    increments: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct EventsPayload {
    events: Vec<EventInput>,
}

#[derive(Debug, Deserialize)]
struct SessionBatch {
    #[serde(default)]
    events: Vec<EventInput>,
    #[serde(default)]
    queue_updates: Vec<QueueUpdate>,
    #[serde(default)]
    stats_increments: Vec<String>,
    #[serde(default)]
    context_append: Vec<String>,
    #[serde(default)]
    dashboard_append: String,
    #[serde(default)]
    checkpoint_write: String,
}

#[derive(Debug, Serialize)]
struct JournalEvent<'a> {
    ts: String,
    event: &'a str,
    message_id: Option<&'a str>,
    data: &'a Value,
}

fn default_status() -> String {
    "pending".into()
}
fn default_approval_state() -> String {
    "none".into()
}
fn default_research_state() -> String {
    "not_started".into()
}
fn default_read_state() -> String {
    "unknown".into()
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .with_target(false)
        .compact()
        .init();

    let cli = Cli::parse();
    if let Err(err) = run(cli).await {
        eprintln!("{err:?}");
        std::process::exit(1);
    }
    Ok(())
}

async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Email { command } => match command {
            EmailCommands::Session { command } => match command {
                SessionCommands::Init {
                    root,
                    gmail_query,
                    account,
                    timezone,
                    session_id,
                } => session_init(&root, &gmail_query, &account, &timezone, &session_id),
                SessionCommands::Apply {
                    session_dir,
                    batch_file,
                    events,
                    queue_updates,
                    stat_increments,
                    context_append,
                    dashboard_append,
                    checkpoint_write,
                    timezone,
                } => session_apply(SessionApplyArgs {
                    session_dir,
                    batch_file,
                    events,
                    queue_updates,
                    stat_increments,
                    context_append,
                    dashboard_append,
                    checkpoint_write,
                    timezone,
                }),
            },
            EmailCommands::Queue { command } => match command {
                QueueCommands::Build {
                    session_dir,
                    items_file,
                    items,
                    replace,
                    timezone,
                } => queue_build(
                    &session_dir,
                    items_file.as_deref(),
                    &items,
                    replace,
                    &timezone,
                ),
                QueueCommands::View {
                    session_dir,
                    status,
                    limit,
                    json,
                } => queue_view(&session_dir, status.as_deref(), limit, json),
                QueueCommands::Update {
                    session_dir,
                    update_file,
                    timezone,
                } => queue_update(&session_dir, &update_file, &timezone),
            },
            EmailCommands::Journal { command } => match command {
                JournalCommands::Event {
                    session_dir,
                    event,
                    session,
                    event_name,
                    message_id,
                    data,
                    increments,
                    set_status,
                    set_approval_state,
                    set_research_state,
                    set_read_state,
                    set_recommended_action,
                    set_terminal_action,
                    set_dashboard_anchor,
                    timezone,
                } => journal_event(JournalEventArgs {
                    session_dir: choose_session_dir(session_dir, session)?,
                    event: choose_event(event, event_name)?,
                    message_id,
                    data,
                    increments,
                    set_status,
                    set_approval_state,
                    set_research_state,
                    set_read_state,
                    set_recommended_action,
                    set_terminal_action,
                    set_dashboard_anchor,
                    timezone,
                }),
                JournalCommands::Batch {
                    session_dir,
                    batch_file,
                    timezone,
                } => journal_batch(&session_dir, &batch_file, &timezone),
            },
        },
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            match shell {
                CompletionShell::Zsh => {
                    clap_complete::generate(
                        clap_complete::Shell::Zsh,
                        &mut cmd,
                        "gtdkit",
                        &mut std::io::stdout(),
                    );
                }
            }
            Ok(())
        }
    }
}

struct SessionApplyArgs {
    session_dir: PathBuf,
    batch_file: Option<PathBuf>,
    events: Vec<String>,
    queue_updates: Vec<String>,
    stat_increments: Vec<String>,
    context_append: Vec<String>,
    dashboard_append: String,
    checkpoint_write: String,
    timezone: String,
}

struct JournalEventArgs {
    session_dir: PathBuf,
    event: String,
    message_id: Option<String>,
    data: String,
    increments: Vec<String>,
    set_status: String,
    set_approval_state: String,
    set_research_state: String,
    set_read_state: String,
    set_recommended_action: String,
    set_terminal_action: String,
    set_dashboard_anchor: String,
    timezone: String,
}

fn choose_session_dir(positional: Option<PathBuf>, flag: Option<PathBuf>) -> Result<PathBuf> {
    match (positional, flag) {
        (Some(positional), Some(flag)) if positional != flag => {
            bail!("provide the session directory either positionally or with --session, not both")
        }
        (Some(path), _) | (_, Some(path)) => Ok(path),
        (None, None) => bail!("missing session directory"),
    }
}

fn choose_event(positional: Option<String>, flag: Option<String>) -> Result<String> {
    match (positional, flag) {
        (Some(positional), Some(flag)) if positional != flag => {
            bail!("provide the event either positionally or with --event, not both")
        }
        (Some(event), _) | (_, Some(event)) => Ok(event),
        (None, None) => bail!("missing event"),
    }
}

fn session_init(
    root: &Path,
    gmail_query: &str,
    account: &str,
    timezone: &str,
    session_id: &str,
) -> Result<()> {
    let root = expand_path(root)?;
    let now = now(timezone)?;
    let session_id = if session_id.is_empty() {
        format!("session-{}", now.format("%Y%m%d-%H%M%S"))
    } else {
        session_id.to_owned()
    };
    let session_dir = root
        .join(format!("{:04}", now.year()))
        .join(format!("{:02}", now.month()))
        .join(format!("{:02}", now.day()))
        .join(session_id);
    std::fs::create_dir_all(&session_dir).into_diagnostic()?;
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
        account: account.into(),
        gmail_query: gmail_query.into(),
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
        gmail_query: gmail_query.into(),
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
        "data": {"session_dir": session_dir.display().to_string(), "gmail_query": gmail_query}
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
            "# Checkpoint\n\nSession: `{}`\n\nNext step: populate `queue.json` from Gmail query `in:inbox`, newest to oldest, then process the first pending item.\n",
            session_dir.display()
        ),
    )?;
    println!("{}", session_dir.display());
    Ok(())
}

fn queue_build(
    session_dir: &Path,
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

    let session_dir = require_session(session_dir)?;
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

fn queue_view(
    session_dir: &Path,
    status: Option<&str>,
    limit: usize,
    json_output: bool,
) -> Result<()> {
    let session_dir = require_session(session_dir)?;
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
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).into_diagnostic()?
        );
        return Ok(());
    }

    println!("Session: {}", session_dir.display());
    println!("Current pointer: {}", queue.current_pointer);
    println!("Queue length: {}", queue.items.len());
    if let Some(status) = status {
        println!("Filter: status={status}");
    }
    println!("Showing: {} item(s)", items.len());
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
        println!("{table}");
    }
    Ok(())
}

fn queue_update(session_dir: &Path, update_file: &Path, timezone: &str) -> Result<()> {
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

    let session_dir = require_session(session_dir)?;
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

fn journal_event(args: JournalEventArgs) -> Result<()> {
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

    let session_dir = require_session(&args.session_dir)?;
    let _lock = SessionLock::acquire(&session_dir)?;
    let now = now(&args.timezone)?;
    if !queue_fields.is_empty() {
        assert_queue_contains(&session_dir, args.message_id.as_deref().unwrap())?;
    }
    append_events(
        &session_dir,
        &[EventInput {
            event: args.event.clone(),
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

fn journal_batch(session_dir: &Path, batch_file: &Path, timezone: &str) -> Result<()> {
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

    let session_dir = require_session(session_dir)?;
    let _lock = SessionLock::acquire(&session_dir)?;
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
    append_events(&session_dir, &operations, &now)?;
    increment_stats(&session_dir, &increments)?;
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

fn session_apply(args: SessionApplyArgs) -> Result<()> {
    let direct_args_used = !args.events.is_empty()
        || !args.queue_updates.is_empty()
        || !args.stat_increments.is_empty()
        || !args.context_append.is_empty()
        || !args.dashboard_append.is_empty()
        || !args.checkpoint_write.is_empty();
    if args.batch_file.is_some() && direct_args_used {
        bail!("use either --batch-file or direct batch parameters, not both");
    }
    if args.batch_file.is_none() && !direct_args_used {
        bail!("provide either --batch-file or direct batch parameters");
    }

    let batch = if let Some(path) = args.batch_file {
        read_json(&expand_path(&path)?)?
    } else {
        SessionBatch {
            events: args
                .events
                .iter()
                .map(|raw| serde_json::from_str(raw).into_diagnostic())
                .collect::<Result<Vec<EventInput>>>()?,
            queue_updates: args
                .queue_updates
                .iter()
                .map(|raw| serde_json::from_str(raw).into_diagnostic())
                .collect::<Result<Vec<QueueUpdate>>>()?,
            stats_increments: args.stat_increments,
            context_append: args.context_append,
            dashboard_append: args.dashboard_append,
            checkpoint_write: args.checkpoint_write,
        }
    };
    validate_events(&batch.events)?;
    validate_queue_updates(&batch.queue_updates, "queue update")?;

    let session_dir = require_session(&args.session_dir)?;
    let _lock = SessionLock::acquire(&session_dir)?;
    let queue: Queue = read_json(&session_dir.join("queue.json"))?;
    let message_ids: BTreeSet<_> = queue
        .items
        .iter()
        .map(|item| item.message_id.as_str())
        .collect();
    for (index, update) in batch.queue_updates.iter().enumerate() {
        if !message_ids.contains(update.message_id.as_str()) {
            bail!(
                "queue update {index} message ID not found in queue: {}",
                update.message_id
            );
        }
    }

    let now = now(&args.timezone)?;
    append_events(&session_dir, &batch.events, &now)?;
    increment_stats(&session_dir, &batch.stats_increments)?;
    if !batch.queue_updates.is_empty() {
        let mut queue: Queue = read_json(&session_dir.join("queue.json"))?;
        apply_queue_updates(
            &mut queue,
            &batch.queue_updates,
            &iso(&now),
            "Message ID not found in queue",
        )?;
        write_json(&session_dir.join("queue.json"), &queue)?;
    }
    append_lines(&session_dir.join("context.md"), &batch.context_append)?;
    if !batch.dashboard_append.is_empty() {
        append_text(
            &session_dir.join("dashboards.md"),
            &(batch.dashboard_append.trim_end_matches('\n').to_owned() + "\n"),
        )?;
    }
    if !batch.checkpoint_write.is_empty() {
        write_text(&session_dir.join("checkpoint.md"), &batch.checkpoint_write)?;
    }
    Ok(())
}

fn append_events(
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
    append_text(&session_dir.join("events.jsonl"), &lines)
}

fn increment_stats(session_dir: &Path, keys: &[String]) -> Result<()> {
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

fn append_lines(path: &Path, lines: &[String]) -> Result<()> {
    if lines.is_empty() {
        return Ok(());
    }
    let text = lines
        .iter()
        .map(|line| line.trim_end_matches('\n'))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    append_text(path, &text)
}

fn apply_queue_updates(
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

fn apply_fields(item: &mut QueueItem, fields: BTreeMap<String, Value>) -> Result<()> {
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

fn validate_events(events: &[EventInput]) -> Result<()> {
    for (index, event) in events.iter().enumerate() {
        if event.event.is_empty() {
            bail!("event {index} missing event");
        }
    }
    Ok(())
}

fn validate_queue_updates(updates: &[QueueUpdate], label: &str) -> Result<()> {
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

fn validate_fields(fields: &BTreeMap<String, Value>, message: &str) -> Result<()> {
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

fn clean_fields(fields: BTreeMap<String, Value>) -> BTreeMap<String, Value> {
    fields
        .into_iter()
        .filter(|(_, value)| match value {
            Value::Null => false,
            Value::String(value) => !value.is_empty(),
            _ => true,
        })
        .collect()
}

fn assert_queue_contains(session_dir: &Path, message_id: &str) -> Result<()> {
    let queue: Queue = read_json(&session_dir.join("queue.json"))?;
    if queue.items.iter().any(|item| item.message_id == message_id) {
        Ok(())
    } else {
        bail!("Message ID not found in queue: {message_id}")
    }
}

fn normalize_queue_items(values: Vec<Value>) -> Result<Vec<QueueItem>> {
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

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let text = std::fs::read_to_string(path)
        .into_diagnostic()
        .map_err(|err| err.wrap_err(format!("failed to read {}", path.display())))?;
    serde_json::from_str(&text)
        .into_diagnostic()
        .map_err(|err| err.wrap_err(format!("failed to parse {}", path.display())))
}

fn read_json_value(path: &Path) -> Result<Value> {
    read_json(path)
}

fn write_json<T: Serialize>(path: &Path, data: &T) -> Result<()> {
    let text = serde_json::to_string_pretty(data).into_diagnostic()? + "\n";
    write_text(path, &text)
}

fn write_text(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).into_diagnostic()?;
    }
    let parent = path
        .parent()
        .ok_or_else(|| miette!("path has no parent: {}", path.display()))?;
    let mut tmp = NamedTempFile::new_in(parent).into_diagnostic()?;
    tmp.write_all(content.as_bytes()).into_diagnostic()?;
    tmp.as_file_mut().sync_all().into_diagnostic()?;
    tmp.persist(path)
        .map_err(|err| miette!("failed to persist {}: {}", path.display(), err.error))?;
    Ok(())
}

fn append_text(path: &Path, content: &str) -> Result<()> {
    let mut existing = String::new();
    if path.exists() {
        File::open(path)
            .into_diagnostic()?
            .read_to_string(&mut existing)
            .into_diagnostic()?;
    }
    existing.push_str(content);
    write_text(path, &existing)
}

fn ensure_file_text(path: &Path, content: &str) -> Result<()> {
    if !path.exists() {
        write_text(path, content)?;
    }
    Ok(())
}

fn ensure_file_json(path: &Path, value: Value) -> Result<()> {
    if !path.exists() {
        write_json(path, &value)?;
    }
    Ok(())
}

fn require_session(path: &Path) -> Result<PathBuf> {
    let path = expand_path(path)?;
    if !path.exists() {
        bail!("Session directory does not exist: {}", path.display());
    }
    Ok(path)
}

fn expand_path(path: &Path) -> Result<PathBuf> {
    let path = if let Some(raw) = path.to_str().filter(|raw| raw.starts_with("~/")) {
        let home = std::env::var("HOME").into_diagnostic()?;
        PathBuf::from(home).join(&raw[2..])
    } else {
        path.to_path_buf()
    };
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(std::env::current_dir().into_diagnostic()?.join(path))
    }
}

struct SessionLock {
    file: File,
}

impl SessionLock {
    fn acquire(session_dir: &Path) -> Result<Self> {
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

fn now(timezone: &str) -> Result<DateTime<FixedOffset>> {
    if !matches!(timezone, DEFAULT_TZ | "US/Central" | "CST6CDT") {
        tracing::warn!(
            timezone,
            "falling back to local offset; named timezone support is intentionally narrow"
        );
    }
    Ok(Local::now().fixed_offset())
}

fn iso(value: &DateTime<FixedOffset>) -> String {
    value.to_rfc3339_opts(SecondsFormat::AutoSi, false)
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

fn newsletter_whitelist() -> Value {
    json!({
        "schema_version": 1,
        "description": "Newsletters the user has explicitly chosen to keep during email inbox processing.",
        "entries": [],
        "entry_schema": {
            "approved_at": "YYYY-MM-DD",
            "domain": "sender-domain.example",
            "notes": "Optional durable preference notes.",
            "publication": "Newsletter name",
            "reason_kept": "Why the user chose to keep it.",
            "sender": "newsletter@example.com"
        }
    })
}

fn knowledge_base() -> Value {
    json!({
        "schema_version": 1,
        "status": "unconfigured",
        "selected_provider": null,
        "canonical_source": "markdown",
        "recommendation": "Use plain Markdown notes as the source of truth, with an optional local vector/search index.",
        "candidate_providers": [
            "Markdown vault plus LanceDB index",
            "Obsidian vault plus Khoj",
            "AnythingLLM",
            "Chroma",
            "Qdrant",
            "sqlite-vec"
        ]
    })
}

const LONG_TERM_MEMORY: &str = r#"# Email Inbox Processing Long-Term Memory

Durable preferences and recurring context for the `email-inbox-processing` skill.

## User Preferences

- Process Gmail inbox to zero, not just a sample.
- Process newest to oldest.
- Process exactly one email at a time.
- Never skip an email; if blocked, mark it blocked or waiting with a reason.
- Commit session state to durable files before and after each journaled step.
- Ask before every external state change. Suggestions are allowed; automatic action is not.
- Sending email is allowed only after the user approves the exact draft.
- Before recommending or drafting a reply, search older conversations with the same person or organization to match tone and gather context.
- Bias toward handling the email there and then. The two-minute rule is a guide, and agent research/drafting time does not count as user effort.
- Research is encouraged: read older Gmail threads, follow substantive content links with browser MCP, search the web, and inspect OmniFocus when helpful.
- Read-only research does not require per-step approval; external state changes always require explicit approval.
- Before archiving, deleting, creating a draft, or sending a reply for an email, mark that email read immediately first.
- When an outgoing email or prior user-sent thread text creates a promise, follow-up, deliverable, or clear implied next action, proactively suggest an OmniFocus Inbox task with rich context.
- Use OmniFocus Inbox by default for new tasks, but suggest a more specific project when a good fit is found.
- Use the existing OmniFocus `Maybe` folder/on-hold project pattern for someday/maybe or incubated items.
- For external waits, suggest OmniFocus items tagged `Waiting` and include a due date for follow-up.
- For coordination-heavy replies, prefer process-centric drafts that make the thread goal, next process, owner, and confirmation path clear.
- Very obvious low-signal mail may get a short dashboard, but still process one email at a time and ask before action.
- At apparent inbox zero, check for fresh inbox mail and continue if new messages arrived.

## Delete / Archive Defaults

- Recommend deletion for Amazon order confirmations and ads unless there is an unresolved delivery, refund, warranty, or accounting issue.
- Recommend archive/reference for non-Amazon order confirmations and receipts unless actionable.
- Recommend deletion for ads and promotional email unless there is a concrete action or reference value.

## Newsletter Preferences

- For newsletters not listed in `/Users/xeed/Documents/SOPs/email-inbox-processing/config/newsletter-whitelist.json`, recommend unsubscribe and delete.
- If the user decides to keep a newsletter, add it to the newsletter whitelist after approval so future runs retain it.
- After a successful unsubscribe, delete the email unless explicitly directed otherwise. Mark it read immediately before deleting it.
- Use browser MCP for approved unsubscribe flows so logged-in state is available.

## Sender And Project Context

Add recurring sender, tone, and OmniFocus placement preferences here after approval.

## Knowledge Base Preferences

- No knowledge-base provider has been selected yet.
- Preferred direction under evaluation: plain Markdown notes as canonical source, with an optional local vector/search index for AI retrieval.

## Proposed Additions Log

At the end of each inbox processing session, scan this file to avoid duplicates, then propose durable additions for approval.
"#;

#[allow(dead_code)]
fn _keep_plan_dependencies_visible() {
    let _ = std::mem::size_of::<Utf8PathBuf>();
    let _ = clap_mangen::Man::new(Cli::command());
}
