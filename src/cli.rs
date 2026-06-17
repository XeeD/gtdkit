use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use crate::constants::{DEFAULT_GMAIL_QUERY, DEFAULT_ROOT, DEFAULT_TZ};

#[derive(Debug, Parser)]
#[command(name = "gtdkit", version, about = "GTD workflow utilities")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum Commands {
    #[command(about = "Generate checked-in documentation artifacts")]
    Docs {
        #[command(subcommand)]
        command: DocsCommands,
    },
    #[command(about = "Manage local email inbox-processing session state")]
    Email {
        #[command(subcommand)]
        command: EmailCommands,
    },
    #[command(about = "Generate shell completions")]
    Completions {
        #[arg(help = "Shell to generate completions for")]
        shell: CompletionShell,
    },
}

#[derive(Debug, Subcommand)]
pub enum DocsCommands {
    #[command(about = "Generate the Markdown CLI reference from Clap metadata")]
    CliReference {
        #[arg(long, default_value = "markdown", help = "Output format")]
        format: DocsFormat,
    },
}

#[derive(Clone, Debug, ValueEnum)]
pub enum DocsFormat {
    Markdown,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum CompletionShell {
    Zsh,
}

#[derive(Debug, Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum EmailCommands {
    #[command(about = "Create or apply durable session-level updates")]
    Session {
        #[command(subcommand)]
        command: SessionCommands,
    },
    #[command(about = "Build, inspect, or update the inbox queue")]
    Queue {
        #[command(subcommand)]
        command: QueueCommands,
    },
    #[command(about = "Append journal events and compatibility batches")]
    Journal {
        #[command(subcommand)]
        command: JournalCommands,
    },
    #[command(about = "Record read-only research-agent output")]
    Research {
        #[command(subcommand)]
        command: ResearchCommands,
    },
    #[command(about = "Record dashboard workflow steps")]
    Step {
        #[command(subcommand)]
        command: StepCommands,
    },
    #[command(about = "Record action approval and completion")]
    Action {
        #[command(subcommand)]
        command: ActionCommands,
    },
    #[command(about = "Record a fresh Gmail inbox check result")]
    FreshCheck(FreshCheckArgs),
}

#[derive(Debug, Subcommand)]
pub enum SessionCommands {
    #[command(about = "Create a dated restartable email session")]
    Init(SessionInitArgs),
    #[command(about = "Apply a low-level multi-file local session update")]
    Apply(SessionApplyArgs),
}

#[derive(Debug, Clone, clap::Args)]
pub struct SessionInitArgs {
    #[arg(value_name = "SESSION_ID", help = "Optional short session ID")]
    pub session_id: Option<String>,
    #[arg(long, default_value = DEFAULT_ROOT, help = "Email SOP root directory")]
    pub root: PathBuf,
    #[arg(long, default_value = DEFAULT_GMAIL_QUERY, help = "Gmail query recorded in manifest")]
    pub gmail_query: String,
    #[arg(long, default_value = "", help = "Account recorded in manifest")]
    pub account: String,
    #[arg(long, default_value = DEFAULT_TZ, help = "Local timezone label")]
    pub timezone: String,
    #[arg(long, help = "Allow another active same-day session")]
    pub allow_active_session: bool,
}

#[derive(Debug, Clone, clap::Args)]
pub struct SessionApplyArgs {
    #[arg(help = "Short session ID")]
    pub session_id: String,
    #[arg(long, default_value = DEFAULT_ROOT, help = "Email SOP root directory")]
    pub root: PathBuf,
    #[arg(long, help = "JSON batch file to apply")]
    pub batch_file: Option<PathBuf>,
    #[arg(long = "event", help = "Event JSON object; repeatable")]
    pub events: Vec<String>,
    #[arg(long = "queue-update", help = "Queue update JSON object; repeatable")]
    pub queue_updates: Vec<String>,
    #[arg(long = "stat-increment", help = "Stats key to increment; repeatable")]
    pub stat_increments: Vec<String>,
    #[arg(
        long = "context-append",
        allow_hyphen_values = true,
        help = "Markdown context line; repeatable"
    )]
    pub context_append: Vec<String>,
    #[arg(
        long,
        default_value = "",
        allow_hyphen_values = true,
        help = "Markdown dashboard text to append"
    )]
    pub dashboard_append: String,
    #[arg(
        long,
        default_value = "",
        allow_hyphen_values = true,
        help = "Checkpoint Markdown replacement"
    )]
    pub checkpoint_write: String,
    #[arg(long, default_value = DEFAULT_TZ, help = "Local timezone label")]
    pub timezone: String,
}

#[derive(Debug, Subcommand)]
pub enum QueueCommands {
    #[command(about = "Create or extend queue entries from supplied Gmail metadata")]
    Build {
        #[arg(help = "Short session ID")]
        session_id: String,
        #[arg(long, default_value = DEFAULT_ROOT, help = "Email SOP root directory")]
        root: PathBuf,
        #[arg(long, help = "JSON file containing queue items")]
        items_file: Option<PathBuf>,
        #[arg(long = "item", help = "Queue item JSON object; repeatable")]
        items: Vec<String>,
        #[arg(long, help = "Replace the existing queue")]
        replace: bool,
        #[arg(long, default_value = DEFAULT_TZ, help = "Local timezone label")]
        timezone: String,
    },
    #[command(about = "Inspect the current queue through the session lock")]
    View {
        #[arg(help = "Short session ID")]
        session_id: String,
        #[arg(long, default_value = DEFAULT_ROOT, help = "Email SOP root directory")]
        root: PathBuf,
        #[arg(long, help = "Filter by queue status")]
        status: Option<String>,
        #[arg(long, default_value_t = 10, help = "Maximum number of items to show")]
        limit: usize,
        #[arg(long, help = "Emit JSON instead of a table")]
        json: bool,
    },
    #[command(about = "Apply queue-only field updates from JSON")]
    Update {
        #[arg(help = "Short session ID")]
        session_id: String,
        #[arg(long, default_value = DEFAULT_ROOT, help = "Email SOP root directory")]
        root: PathBuf,
        #[arg(long, help = "JSON update payload")]
        update_file: PathBuf,
        #[arg(long, default_value = DEFAULT_TZ, help = "Local timezone label")]
        timezone: String,
    },
}

#[derive(Debug, Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum JournalCommands {
    #[command(about = "Append one journal event and optional queue/stat updates")]
    Event(JournalEventArgs),
    #[command(about = "Append a compatibility batch of journal events")]
    Batch {
        #[arg(help = "Short session ID")]
        session_id: String,
        #[arg(long, default_value = DEFAULT_ROOT, help = "Email SOP root directory")]
        root: PathBuf,
        #[arg(long, help = "JSON event batch file")]
        batch_file: PathBuf,
        #[arg(long, default_value = DEFAULT_TZ, help = "Local timezone label")]
        timezone: String,
    },
}

#[derive(Debug, Clone, clap::Args)]
pub struct JournalEventArgs {
    #[arg(value_name = "SESSION_ID", help = "Short session ID")]
    pub session_id: String,
    #[arg(
        value_name = "EVENT",
        required_unless_present = "event_name",
        help = "Event name"
    )]
    pub event: Option<String>,
    #[arg(long, default_value = DEFAULT_ROOT, help = "Email SOP root directory")]
    pub root: PathBuf,
    #[arg(
        long = "event",
        value_name = "EVENT",
        help = "Event name alternative to positional"
    )]
    pub event_name: Option<String>,
    #[arg(long, help = "Queue message ID associated with the event")]
    pub message_id: Option<String>,
    #[arg(
        long,
        default_value = "",
        allow_hyphen_values = true,
        help = "Event data JSON object"
    )]
    pub data: String,
    #[arg(long = "increment", help = "Stats key to increment; repeatable")]
    pub increments: Vec<String>,
    #[arg(long, default_value = "", help = "Set queue status")]
    pub set_status: String,
    #[arg(long, default_value = "", help = "Set queue approval_state")]
    pub set_approval_state: String,
    #[arg(long, default_value = "", help = "Set queue research_state")]
    pub set_research_state: String,
    #[arg(long, default_value = "", help = "Set queue read_state")]
    pub set_read_state: String,
    #[arg(
        long,
        default_value = "",
        allow_hyphen_values = true,
        help = "Set queue recommended_action"
    )]
    pub set_recommended_action: String,
    #[arg(
        long,
        default_value = "",
        allow_hyphen_values = true,
        help = "Set queue terminal_action"
    )]
    pub set_terminal_action: String,
    #[arg(long, default_value = "", help = "Set queue dashboard_anchor")]
    pub set_dashboard_anchor: String,
    #[arg(long, default_value = DEFAULT_TZ, help = "Local timezone label")]
    pub timezone: String,
}

#[derive(Debug, Subcommand)]
pub enum ResearchCommands {
    #[command(about = "Record a read-only subagent digest")]
    Digest(ResearchDigestArgs),
}

#[derive(Debug, Clone, clap::Args)]
pub struct ResearchDigestArgs {
    #[arg(help = "Short session ID")]
    pub session_id: String,
    #[arg(long, default_value = DEFAULT_ROOT, help = "Email SOP root directory")]
    pub root: PathBuf,
    #[arg(long, help = "Queue message ID researched")]
    pub message_id: String,
    #[arg(long, help = "Queue index researched")]
    pub queue_index: Option<usize>,
    #[arg(long, default_value = "", help = "Research agent identifier")]
    pub agent_id: String,
    #[arg(long, default_value = "", help = "Research agent display name")]
    pub agent_name: String,
    #[arg(
        long,
        default_value = "",
        allow_hyphen_values = true,
        help = "Recommended action from digest"
    )]
    pub recommended_action: String,
    #[arg(long, help = "Assert the subagent performed no mutations")]
    pub no_mutations_performed: bool,
    #[arg(
        long,
        default_value = "buffered",
        help = "Queue research_state to store"
    )]
    pub state: String,
    #[arg(
        long,
        default_value = "",
        allow_hyphen_values = true,
        help = "Digest text or compact JSON"
    )]
    pub digest: String,
    #[arg(
        long,
        help = "Backfill Gmail thread ID from the read-only research result"
    )]
    pub thread_id: Option<String>,
    #[arg(
        long,
        help = "Backfill message internal date from the read-only research result"
    )]
    pub internal_date: Option<String>,
    #[arg(
        long,
        allow_hyphen_values = true,
        help = "Backfill message From header"
    )]
    pub from: Option<String>,
    #[arg(long, allow_hyphen_values = true, help = "Backfill message subject")]
    pub subject: Option<String>,
    #[arg(long, allow_hyphen_values = true, help = "Backfill message snippet")]
    pub snippet: Option<String>,
    #[arg(long = "label", help = "Backfill Gmail label; repeatable")]
    pub labels: Vec<String>,
    #[arg(long, default_value = DEFAULT_TZ, help = "Local timezone label")]
    pub timezone: String,
}

#[derive(Debug, Subcommand)]
pub enum StepCommands {
    #[command(about = "Record a rendered dashboard and approval request")]
    Dashboard(DashboardArgs),
}

#[derive(Debug, Clone, clap::Args)]
pub struct DashboardArgs {
    #[arg(help = "Short session ID")]
    pub session_id: String,
    #[arg(long, default_value = DEFAULT_ROOT, help = "Email SOP root directory")]
    pub root: PathBuf,
    #[arg(long, help = "Queue message ID dashboarded")]
    pub message_id: String,
    #[arg(long, help = "Stable dashboard anchor stored in session metadata")]
    pub dashboard_anchor: String,
    #[arg(
        long,
        default_value = "",
        allow_hyphen_values = true,
        help = "Recommended action shown to the user"
    )]
    pub recommended_action: String,
    #[arg(long, value_delimiter = ',', help = "Comma-separated approval options")]
    pub approval_options: Vec<String>,
    #[arg(
        long,
        default_value = "",
        allow_hyphen_values = true,
        help = "Queue read_state to store"
    )]
    pub read_state: String,
    #[arg(long, help = "Read dashboard Markdown from stdin")]
    pub dashboard_stdin: bool,
    #[arg(
        long,
        default_value = "",
        allow_hyphen_values = true,
        help = "Dashboard Markdown text"
    )]
    pub dashboard_text: String,
    #[arg(long, help = "Also journal email_started")]
    pub email_started: bool,
    #[arg(long, default_value = DEFAULT_TZ, help = "Local timezone label")]
    pub timezone: String,
}

#[derive(Debug, Subcommand)]
pub enum ActionCommands {
    #[command(about = "Record explicit user approval for an action")]
    Approve(ActionApproveArgs),
    #[command(about = "Record a completed terminal action after verification")]
    Complete(ActionCompleteArgs),
}

#[derive(Debug, Clone, clap::Args)]
pub struct ActionApproveArgs {
    #[arg(help = "Short session ID")]
    pub session_id: String,
    #[arg(long, default_value = DEFAULT_ROOT, help = "Email SOP root directory")]
    pub root: PathBuf,
    #[arg(long, help = "Queue message ID approved")]
    pub message_id: String,
    #[arg(long, help = "Approved action")]
    pub action: String,
    #[arg(
        long,
        default_value = "",
        allow_hyphen_values = true,
        help = "User confirmation text"
    )]
    pub user_reply: String,
    #[arg(long, default_value = DEFAULT_TZ, help = "Local timezone label")]
    pub timezone: String,
}

#[derive(Debug, Clone, clap::Args)]
pub struct ActionCompleteArgs {
    #[arg(help = "Short session ID")]
    pub session_id: String,
    #[arg(long, default_value = DEFAULT_ROOT, help = "Email SOP root directory")]
    pub root: PathBuf,
    #[arg(long, help = "Queue message ID completed")]
    pub message_id: String,
    #[arg(long, help = "Terminal queue status/action")]
    pub terminal_action: String,
    #[arg(
        long,
        default_value = "",
        help = "External Gmail action recorded as metadata"
    )]
    pub gmail_action: String,
    #[arg(long = "stat", help = "Stats key to increment; repeatable")]
    pub stats: Vec<String>,
    #[arg(
        long,
        default_value = "",
        allow_hyphen_values = true,
        help = "Verification note"
    )]
    pub verification: String,
    #[arg(long, default_value = DEFAULT_TZ, help = "Local timezone label")]
    pub timezone: String,
}

#[derive(Debug, Clone, clap::Args)]
pub struct FreshCheckArgs {
    #[arg(help = "Short session ID")]
    pub session_id: String,
    #[arg(long, default_value = DEFAULT_ROOT, help = "Email SOP root directory")]
    pub root: PathBuf,
    #[arg(long, help = "Fresh inbox message count")]
    pub count: usize,
    #[arg(long = "message-id", help = "Fresh message ID; repeatable")]
    pub message_ids: Vec<String>,
    #[arg(long, default_value = DEFAULT_TZ, help = "Local timezone label")]
    pub timezone: String,
}
