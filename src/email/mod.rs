pub(crate) mod apply;
pub(crate) mod journal;
pub(crate) mod model;
pub(crate) mod queue;
pub(crate) mod session;
pub(crate) mod workflow;

use miette::Result;

use crate::cli::{
    ActionCommands, EmailCommands, JournalCommands, QueueCommands, ResearchCommands,
    SessionCommands, StepCommands,
};

pub(crate) fn run(command: EmailCommands) -> Result<()> {
    match command {
        EmailCommands::Session { command } => match command {
            SessionCommands::Init(args) => session::init(args),
            SessionCommands::Apply(args) => apply::session_apply(args),
        },
        EmailCommands::Queue { command } => match command {
            QueueCommands::Build {
                session_id,
                root,
                items_file,
                items,
                replace,
                timezone,
            } => queue::build(
                &session_id,
                &root,
                items_file.as_deref(),
                &items,
                replace,
                &timezone,
            ),
            QueueCommands::View {
                session_id,
                root,
                status,
                limit,
                json,
            } => queue::view(&session_id, &root, status.as_deref(), limit, json),
            QueueCommands::Update {
                session_id,
                root,
                update_file,
                timezone,
            } => queue::update(&session_id, &root, &update_file, &timezone),
        },
        EmailCommands::Journal { command } => match command {
            JournalCommands::Event(args) => journal::event(args),
            JournalCommands::Batch {
                session_id,
                root,
                batch_file,
                timezone,
            } => journal::batch(&session_id, &root, &batch_file, &timezone),
        },
        EmailCommands::Research { command } => match command {
            ResearchCommands::Digest(args) => workflow::research_digest(args),
        },
        EmailCommands::Step { command } => match command {
            StepCommands::Dashboard(args) => workflow::dashboard(args),
        },
        EmailCommands::Action { command } => match command {
            ActionCommands::Approve(args) => workflow::action_approve(args),
            ActionCommands::Complete(args) => workflow::action_complete(args),
        },
        EmailCommands::FreshCheck(args) => workflow::fresh_check(args),
    }
}
