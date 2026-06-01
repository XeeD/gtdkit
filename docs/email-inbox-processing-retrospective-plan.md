# Email Inbox Processing Retrospective Implementation Plan

This is a handoff prompt for a future agent or engineer. It records the context,
failure modes, decisions, and implementation plan from the 2026-05-31 Gmail
inbox-processing session. The goal is to improve the `$email-inbox-processing`
SOP and `gtdkit` so future email processing is safe, fast, token-efficient, and
easy to resume.

## Context

Session: `email-20260531-1133`

Session root:

```text
/Users/xeed/Library/Mobile Documents/com~apple~CloudDocs/SOPs/email-inbox-processing/2026/05/31/email-20260531-1133
```

The original inbox queue had five messages:

- Matt Hampton / First Tech mortgage thread.
- Paul Mak / Waterfront audited accounts thread.
- Breville+ sous vide cleaning reminder.
- Luke Drewett / Chestertons Flat 1305 sale terms.
- Building Kidz Connect login issue thread.

After the original queue drained, a fresh `in:inbox` check found two new Aubrey
emails about 2320 Via Cordova Ct.

The user's operating goal is strict inbox-zero processing with:

- one user-facing dashboard at a time;
- explicit approval before every Gmail, browser, OmniFocus, file, whitelist,
  memory, or knowledge-base mutation;
- read-only research allowed ahead of the current dashboard;
- research-agent-first email reads to save tokens;
- durable session files as the source of truth.

The user explicitly clarified during this session:

- The main agent should not read email bodies as normal operation.
- Research agents should perform initial email body/thread reads.
- The main agent may read email bodies only when explicitly or contextually
  prompted by the user for that specific case.
- The user is willing to wait up to 1 minute for a research agent result before
  a dashboard timeout/pause.
- The user dislikes visible XML-style dashboard tags such as
  `<email_dashboard>`.
- The user wants the `gtdkit` CLI reference to live in the `gtdkit` repo, be
  generated from a single source of truth, and be loaded by the email processing
  skill.
- The user likes an explicit doc regeneration command plus README/AGENTS
  guidance, rather than build-time repo mutation.

## Observed Failure Modes

### Main-Agent Body Read Before Research Digest

After queue build and research-agent assignment, the main agent locally read
Matt Hampton's thread before waiting for the research agent digest. This was
read-only, but it violated the cost-control model. The user corrected this:
initial body reads should be done by the cheaper research agent, not by the main
agent, unless the user asks.

Required SOP fix: after queue build and subagent assignment, the main agent must
not read queued message bodies while a research assignment exists or is pending,
unless the user explicitly authorizes a main-agent read for that email.

### "Stop" Misinterpreted As Cancel Research Agents

The user said "Stop right here" after the main-agent body read issue. The main
agent interpreted this too literally and killed the running Chestertons research
agent. The correct interpretation was: stop local main-agent reads/processing
and wait for research agents.

Required SOP fix: unless the user explicitly says "cancel", "kill", or "stop the
agents", a stop/pause command must leave read-only research agents running.

### Too-Short Research Wait

The main agent waited 30 seconds for the fresh Aubrey research agents and then
paused. The user said they are willing to wait 1 minute.

Confirmed tool behavior: `multi_agent_v1.wait_agent` supports configurable
`timeout_ms`, including `60000`.

Required SOP fix: wait up to 60 seconds for the first research digest before
pausing at the dashboard boundary. Do not fall back to local body reads just
because the timeout expires.

### Agent Thread Limit

Completed old research agents were left open. Later, while processing fresh
Aubrey mail, spawning the second new research agent failed with an agent thread
limit error until completed agents were closed.

Required SOP fix: after a research digest is journaled and no follow-up to that
subagent is needed, close the agent.

### gtdkit CLI Discovery Overhead

The main agent repeatedly invoked `gtdkit ... --help` to learn command shapes,
especially `session apply`, `queue build`, and `queue update`. This wasted
tokens and turn time.

Root cause: the email-processing skill loads SOP/state docs but not a complete
`gtdkit` CLI command reference.

Required fix: make a generated CLI reference part of the `gtdkit` repo and
update the skill to load it.

### Temp JSON Files For Batch Updates

The main agent created temporary JSON files such as:

```text
/private/tmp/email-20260531-1133-paul-dashboard-batch.json
/private/tmp/email-20260531-1133-matt-dashboard-batch.json
/private/tmp/email-20260531-1133-chestertons-dashboard-batch.json
```

Then it invoked `gtdkit email session apply --batch-file ...`.

This worked, but it is not the desired ergonomics. The user wants the agent to
invoke the CLI with arguments, not write an intermediate file and then invoke
the CLI. The existing direct `--event`, `--queue-update`, `--stat-increment`,
`--context-append`, `--dashboard-append`, and `--checkpoint-write` interface can
work for simple cases but becomes cumbersome for rich dashboard/action steps.

Required `gtdkit` fix: add higher-level commands for common email-processing
steps so agents do not need ad hoc JSON files for normal workflow.

### Gmail Thread Archive Gotcha

After replying to Matt, the workflow archived only one Gmail message ID. The
Gmail UI still showed the thread with an `Inbox` chip, causing confusion.
Additional archive attempts by subject returned success through the API, but
the UI behavior was still confusing because Gmail conversation labels and
Important/Priority Inbox display do not map cleanly to one message ID.

Facts observed:

- Gmail archive is label removal, not a move into a separate archive folder.
- Gmail conversation UI can show labels at thread/conversation level.
- A single message archive action is not always sufficient for the user's UI
  expectation of "the conversation is gone from inbox."
- The sent message did not show `INBOX` in API output, but later manual UI
  archive changed its label set from `SENT` to `IMPORTANT, SENT`, further
  showing the difference between Inbox and Important/Priority display.

Required SOP fix: document conversation-level archive gotchas and verification
steps. When closing a conversation, prefer removing `INBOX` from all
inbox-labelled messages in the target thread/conversation, not just the current
message ID, and record verification. If the UI disagrees with API search,
inspect per-message labels and distinguish `INBOX` from `IMPORTANT`.

### Visible XML Dashboard Tags

Dashboards were rendered with visible wrappers like:

```text
<email_dashboard id="email-0004">
...
</email_dashboard>
```

The user said they dislike this. Dashboard IDs are useful for journaling and
resumability, but should not be visible in chat.

Required SOP fix: render user-facing dashboards as plain Markdown only. Store
dashboard anchor IDs in session metadata/journal/dashboards.md if needed, not
as visible XML tags.

### Dashboard Verbosity

Some dashboards were too verbose for simple archive/delete decisions. The full
dashboard is useful for complex messages, but low-risk ads, receipts, and
handled admin messages need a compact form.

Required SOP fix: define a compact dashboard variant for low-risk messages.

## Decisions Already Made

- CLI reference should be generated from `gtdkit`, not hand-maintained in the
  skill.
- Use an explicit regeneration command, not build-time repo mutation.
- Check in the generated Markdown artifact.
- README and AGENTS should say to regenerate CLI docs whenever CLI commands,
  flags, status values, schemas, session/journal behavior, or workflow command
  behavior changes.
- Add a test that fails if generated CLI docs differ from the checked-in file.
- Generator should be a `gtdkit` command.
- Use `timeout_ms: 60000` for research-agent waits.
- Remove visible XML tags from dashboards.

## gtdkit Implementation Plan

### CLI Reference Generation

Add:

```sh
gtdkit docs cli-reference --format markdown
```

Use Clap metadata as the source of truth. Do not shell out to `gtdkit --help`
and parse terminal help text. Prefer walking `Cli::command()` and its
subcommands/options directly.

Suggested implementation shape:

- Add a top-level `Docs` command with subcommand `CliReference`.
- Add `--format markdown`. Only Markdown is required initially.
- Generate a concise command reference that includes:
  - command tree;
  - purpose/description from Clap metadata;
  - arguments and flags;
  - defaults where Clap exposes them;
  - repeatable flags;
  - examples for email session, queue, journal, and docs commands.
- Check in the generated output at:

```text
docs/cli-reference.md
```

Do not generate into `docs/cli-reference.md` during `cargo build`. Build scripts
should not mutate repo-tracked files. `OUT_DIR` generation is possible but not
useful for the skill because it lacks a stable path.

Add README and AGENTS guidance:

```text
When changing CLI commands, flags, status values, file schemas, session/journal
behavior, or workflow command semantics, regenerate the CLI reference:

cargo run -- docs cli-reference --format markdown > docs/cli-reference.md

Include the updated docs in the same change.
```

Add an equivalent installed-command example:

```sh
gtdkit docs cli-reference --format markdown > docs/cli-reference.md
```

Add a test, for example `cli_reference_doc_is_current`, that generates the
Markdown in memory and compares it to `docs/cli-reference.md`. Normalize line
endings if needed. This gives the user the "single source of truth" behavior
without surprising build-time mutation.

### Ergonomic Email Workflow Commands

Add higher-level `gtdkit email` commands so normal inbox processing does not
require ad hoc JSON batch files.

Keep existing low-level commands for compatibility:

- `gtdkit email session apply`
- `gtdkit email journal event`
- `gtdkit email journal batch`
- `gtdkit email queue update`

Add higher-level commands around common state transitions. Exact names can be
adjusted during implementation, but avoid requiring intermediate files for
normal use.

Required capabilities:

1. Record/buffer a research digest:

```sh
gtdkit email research digest SESSION_ID \
  --message-id MID \
  --queue-index 3 \
  --agent-id AGENT \
  --agent-name NAME \
  --recommended-action archive \
  --no-mutations-performed true \
  --state buffered
```

Effects:

- journal `subagent_digest_received`;
- optionally journal `research_buffered`;
- update queue `research_state`;
- set `recommended_action` when supplied.

2. Start a dashboard step from a digest:

```sh
gtdkit email step dashboard SESSION_ID \
  --message-id MID \
  --dashboard-anchor email-0004 \
  --recommended-action archive \
  --approval-options archive,create-task,waiting \
  --read-state read \
  --dashboard-file-or-stdin ...
```

Preferred input should avoid temp files. Consider stdin for long dashboard text:

```sh
gtdkit email step dashboard SESSION_ID ... --dashboard-stdin
```

Effects:

- journal `email_started` if requested or if not already current;
- journal `research_completed`;
- append dashboard to `dashboards.md`;
- update queue to `waiting_for_user`;
- journal `dashboard_rendered` and `approval_requested`;
- write checkpoint.

3. Record action approval:

```sh
gtdkit email action approve SESSION_ID \
  --message-id MID \
  --action archive \
  --user-reply "archive"
```

Effects:

- journal `action_approved`;
- update queue `approval_state=approved` if desired.

4. Complete a terminal action:

```sh
gtdkit email action complete SESSION_ID \
  --message-id MID \
  --terminal-action archived \
  --gmail-action archive \
  --stat archived \
  --verification "in:inbox subject query returned zero"
```

Effects:

- journal `action_completed`;
- update queue terminal status/action;
- increment stats;
- optionally write checkpoint.

5. Record fresh mail check:

```sh
gtdkit email fresh-check SESSION_ID \
  --count 2 \
  --message-id MID1 \
  --message-id MID2
```

Effects:

- journal `fresh_mail_check`;
- if queue is extended separately, journal `inbox_reconciled`.

The exact command surface should be documented in the generated CLI reference.

### Thread Archive Support

Consider adding a helper state command or documentation support for thread-level
archive verification, but do not make `gtdkit` mutate Gmail. `gtdkit` should
remain local-first. It can record:

- which Gmail message IDs were archived;
- whether the archive was single-message or thread-wide;
- verification query/result;
- UI/API discrepancy notes.

Do not add Gmail API calls to `gtdkit`.

### Rust Code Organization And Comment Quality

The current Rust implementation is concentrated in one large file:

```text
src/main.rs
```

At the time of this plan it was 1,696 lines and contained the CLI definitions,
email command dispatch, session discovery, queue operations, journal/session
mutation logic, filesystem helpers, embedded default JSON data, and tests. That
shape made the first migration fast, but it is now a maintenance risk. The next
implementation pass should split the code before or alongside adding the new
CLI/documentation features so the email workflow does not become harder to
change.

Suggested module split:

- `src/main.rs`: binary entrypoint only; parse `Cli` and call `gtdkit::run`.
- `src/lib.rs`: public crate wiring, shared `Result`, and top-level `run`.
- `src/cli.rs`: Clap `Cli`, command enums, argument structs, and shell
  completion dispatch.
- `src/docs.rs`: CLI reference generation from Clap metadata.
- `src/email/mod.rs`: email command routing.
- `src/email/session.rs`: session init, resolution, active-session discovery,
  locking, checkpoints, and session ID validation.
- `src/email/queue.rs`: queue data model, build/view/update logic, field
  validation, and queue normalization.
- `src/email/journal.rs`: event structs, journal append/batch validation, and
  event timestamp handling.
- `src/email/workflow.rs`: the new higher-level research/dashboard/action/fresh
  check commands.
- `src/fs.rs`: JSON/text read/write/append helpers and path expansion.
- `src/defaults.rs`: default stats, newsletter whitelist, and knowledge-base
  seed data.

Keep this refactor behavior-preserving except where it directly supports the
new commands. Use `pub(crate)` by default and only widen visibility when tests
or command wiring require it. Move tests next to the module owning the behavior,
with integration tests covering the command-line surface.

Improve comments as part of the split. The goal is not more comments everywhere;
the goal is better comments around non-obvious contracts:

- why session IDs are short `email-YYYYMMDD-HHMM` values resolved under the
  iCloud SOP root;
- which functions mutate durable session state and which only validate or
  normalize;
- lock/atomicity expectations for session writes;
- queue field/status invariants that future workflow commands must preserve;
- why generated CLI docs are checked in but not produced by `cargo build`;
- Gmail archive metadata limits, especially that `gtdkit` records local state
  but does not call Gmail.

Avoid comments that merely restate the next line of code. Prefer comments that
name invariants, external contracts, and reasons that are easy to forget.

## Email Inbox Processing Skill / SOP Plan

Update the skill and references under:

```text
/Users/xeed/.agents/skills/email-inbox-processing
```

and the mirrored iCloud SOP files if that is the authoritative user-facing SOP
location for this setup.

### Always Load

Add the generated `gtdkit` CLI reference to the skill's Always Load list:

```text
/Users/xeed/oss-clones/gtdkit/docs/cli-reference.md
```

or, if the skill should not depend on the clone path long-term, document the
installed/reference path and how it is kept current. The user's preference in
this session was to make the reference part of the `gtdkit` repo and point the
skill at that.

### Main-Agent Body Read Rule

Add a non-negotiable rule:

```text
After the queue is built and read-only research agents are assigned, the main
agent MUST NOT read queued email bodies while a research assignment for that
email is pending or available, unless the user explicitly or contextually asks
the main agent to read that email. The normal path is: research agent reads,
main agent journals digest, main agent renders dashboard from digest.
```

Add the reason:

```text
This preserves token efficiency by using the cheaper model for initial reads and
keeps the main agent focused on decisioning, approvals, and state changes.
```

### Stop/Pause Semantics

Add:

```text
If the user says "stop", "pause", or "stop right here", default to stopping
main-agent processing and mutations only. Do not cancel read-only research
agents unless the user explicitly says to cancel, kill, stop the agents, or
discard their work.
```

### Research Wait Timeout

Add:

```text
When waiting for the first completed research digest, use
`multi_agent_v1.wait_agent` with `timeout_ms: 60000`. If no digest returns in
60 seconds, pause at the dashboard boundary and report that research is still
running. Do not read the email locally as a timeout fallback.
```

### Agent Lifecycle

Add:

```text
After a subagent digest is journaled and no follow-up to that agent is needed,
close the subagent to avoid hitting the agent thread limit during fresh-mail
checks.
```

### Dashboard Style

Replace the visible XML dashboard wrapper with plain Markdown.

Old user-facing shape:

```text
<email_dashboard id="email-0004">
...
</email_dashboard>
```

New user-facing shape:

```markdown
**From:** ...
**Subject:** ...
**Received:** ...
**Queue:** ...

**Summary**
...

**Decision**
Recommended action: `archive`

**Approval Options**
1. `archive`
2. `create task: <task>`
3. `waiting: <reason>`
```

Keep `dashboard_anchor` or `email-0004` in journal/queue metadata and
`dashboards.md` if useful, but do not show XML tags to the user.

Add a compact dashboard variant for obvious low-risk items:

- From
- Subject
- one-sentence summary
- classification
- recommended action and reason
- approval options

Research details should be included only when they materially affect the
decision or the email is complex/high-risk.

### Gmail Thread Archive Gotcha

Add a dedicated subsection:

```text
Gmail Archive / Thread Gotcha

Gmail archive means removing the `INBOX` label. The Gmail UI can display labels
at conversation level, while Gmail tools often mutate individual message IDs.
When closing a conversation, archive all relevant inbox-labelled messages in the
thread, not only the latest message ID. Verify with `in:inbox` search and, when
the browser UI disagrees, inspect per-message labels and distinguish `INBOX`
from `IMPORTANT`.

Important/Priority Inbox display is separate from Inbox. A conversation may
still appear in an Important section even after `INBOX` is removed.
```

Add a recommended verification recipe:

1. Search `in:inbox` for the exact subject or thread identifier.
2. If search returns nothing but UI still shows an Inbox chip, read thread
   labels per message.
3. If messages have `IMPORTANT` but not `INBOX`, explain that archive succeeded
   and the remaining UI placement is Important/Priority, not Inbox.
4. If any message still has `INBOX`, archive those specific message IDs.
5. Journal a `gmail_action_verified` or `correction_note` event.

### Correction / Incident Journaling

Add event guidance for workflow corrections:

```text
When correcting a workflow mistake or external-state discrepancy, journal a
short `correction_note` or `incident_note` event with:

- what happened;
- user-visible symptom;
- corrective action;
- verification result;
- whether external state was mutated.
```

This would have captured the Matt archive/UI confusion more clearly.

### No Temp JSON Files In Normal Flow

Add:

```text
Normal inbox processing SHOULD NOT create ad hoc temp JSON files just to call
`gtdkit`. Prefer direct CLI arguments, stdin-oriented high-level commands, or
the new ergonomic `gtdkit email ...` step/action commands. Temporary files are
acceptable only when the CLI explicitly requires a file and no ergonomic command
exists yet; journal or explain the exception when it happens.
```

## Testing And Verification Plan

### gtdkit Tests

Run:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Add test coverage for:

- `gtdkit docs cli-reference --format markdown` output.
- checked-in `docs/cli-reference.md` matches generated output.
- behavior-preserving module split of the current single-file implementation,
  using existing tests before and after the refactor as the guardrail.
- high-level email commands apply all expected journal, queue, stats, dashboard,
  and checkpoint changes atomically.
- high-level commands validate message IDs before mutation.
- failed high-level commands leave session state unchanged.
- stdin dashboard append path handles multi-line Markdown.
- no unsupported queue fields are accepted.
- module-level tests for session ID parsing/resolution, queue field validation,
  event validation, and filesystem helper behavior after the split.

### SOP Verification

Manually verify:

- Skill Always Load list includes `docs/cli-reference.md`.
- SOP says main agent must wait for research digest and not locally read bodies
  by default.
- SOP says 60-second research wait.
- SOP says close completed agents.
- SOP no longer instructs visible XML dashboard wrappers.
- SOP documents Gmail thread/archive/Important gotcha.

### Acceptance Criteria

The implementation is complete when:

- A future agent can process inbox email without calling `gtdkit --help` for
  normal command discovery.
- The Rust implementation is no longer one large `src/main.rs`; major
  responsibilities are split into focused modules with behavior covered by
  tests.
- Comments document the important contracts and mutation boundaries, not just
  obvious control flow.
- A future agent does not need to write ad hoc JSON temp files for normal
  dashboard/action state transitions.
- A future agent will wait up to 60 seconds for research agents and will not
  locally read queued email bodies unless the user authorizes it.
- Completed subagents are closed after digest journaling.
- User-facing dashboards are plain Markdown with no XML tags.
- Gmail archive behavior is documented well enough to avoid repeating the Matt
  thread confusion.
- CLI reference docs are generated from Clap metadata, checked in, and protected
  by tests.

## Prompt For Future Implementing Agent

Use this prompt to execute the work:

```text
You are implementing the email inbox-processing retrospective fixes from
`docs/email-inbox-processing-retrospective-plan.md` in `/Users/xeed/oss-clones/gtdkit`
and the local `$email-inbox-processing` skill.

Do not mutate Gmail, browser state, OmniFocus, newsletter whitelist, long-term
memory, or knowledge-base files. This task is repo/SOP/tooling work only.

Implement:

1. In `gtdkit`, add `gtdkit docs cli-reference --format markdown`, generated
   from Clap metadata rather than parsed `--help` output.
2. Check in `docs/cli-reference.md`.
3. Add tests that fail when the generated CLI reference differs from the
   checked-in Markdown.
4. Update `README.md` and `AGENTS.md` to require regenerating the CLI reference
   whenever CLI commands, flags, schemas, status values, session/journal
   behavior, or workflow command semantics change.
5. Add ergonomic `gtdkit email` high-level commands for common inbox-processing
   state transitions so normal workflow does not require ad hoc JSON temp files.
   Preserve existing low-level commands.
6. Split the current one-file Rust implementation into focused modules before
   the new workflow code makes it larger. Preserve behavior, keep visibility
   narrow, and move tests to the module that owns the behavior.
7. Improve comments around session resolution, durable mutations, locks,
   queue/status invariants, generated-docs policy, and Gmail archive metadata
   limits. Do not add noise comments that restate obvious code.
8. Update the `$email-inbox-processing` skill/SOP to load the generated CLI
   reference and to include the new research-agent-first, stop/pause,
   60-second wait, agent lifecycle, plain Markdown dashboard, Gmail archive
   gotcha, correction journaling, and no-temp-JSON rules.

Use existing repo style and keep filesystem/session mutations inside `gtdkit`
locked/atomic helpers. Do not rewrite unrelated workflow behavior.

Validate with:

cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test

Report the changed files, generated docs command, and any SOP paths updated.
```
