use std::collections::BTreeMap;

use serde_json::{Value, json};

pub(crate) fn base_stats() -> BTreeMap<String, i64> {
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

pub(crate) fn newsletter_whitelist() -> Value {
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

pub(crate) fn knowledge_base() -> Value {
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

pub(crate) const LONG_TERM_MEMORY: &str = r#"# Email Inbox Processing Long-Term Memory

Durable preferences and recurring context for the `email-inbox-processing` skill.

## User Preferences

- Process Gmail inbox to zero, not just a sample.
- Process newest to oldest.
- Present exactly one dashboard and take at most one terminal action at a time.
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

- For newsletters not listed in `/Users/xeed/Library/Mobile Documents/com~apple~CloudDocs/SOPs/email-inbox-processing/config/newsletter-whitelist.json`, recommend unsubscribe and delete.
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
