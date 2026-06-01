use std::collections::BTreeSet;

use miette::{IntoDiagnostic, Result, bail};

use crate::{
    cli::SessionApplyArgs,
    fs_store::{append_text, expand_path, read_json, write_json, write_text},
    time::{iso, now},
};

use super::{
    journal::{append_events, increment_stats, validate_events},
    model::{EventInput, Queue, QueueUpdate, SessionBatch},
    queue::{apply_queue_updates, validate_queue_updates},
    session::{SessionLock, resolve_session},
};

/// Apply one multi-file local session update under the session lock.
///
/// This is the low-level compatibility path for known batches. It validates
/// event shape, queue field names, and target message IDs before appending
/// events, updating stats, rewriting queue state, or touching Markdown files.
/// New workflow commands should wrap this behavior with more semantic inputs
/// rather than forcing agents to prepare ad hoc JSON batch files.
pub(crate) fn session_apply(args: SessionApplyArgs) -> Result<()> {
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

    let session_dir = resolve_session(&args.root, &args.session_id)?;
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

/// Append newline-normalized Markdown lines to a session text artifact.
fn append_lines(path: &std::path::Path, lines: &[String]) -> Result<()> {
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
