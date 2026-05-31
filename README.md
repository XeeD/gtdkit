# gtdkit

`gtdkit` is a Rust command-line toolkit for local GTD workflow state.

The current release focuses on email inbox-processing sessions: creating a
restartable session directory, building and inspecting the message queue,
recording journal events, applying durable session updates, and generating shell
completions.

The CLI is intentionally local-first. It manages structured files on disk and
does not read Gmail, mutate remote services, or make task-management decisions
itself. Agents and other workflow tools use `gtdkit` as the durable state layer
around those external actions.

## Commands

```sh
gtdkit email session init
gtdkit email session apply email-YYYYMMDD-HHMM --batch-file batch.json
gtdkit email queue build email-YYYYMMDD-HHMM --items-file inbox-items.json --replace
gtdkit email queue view email-YYYYMMDD-HHMM --status pending --json
gtdkit email queue update email-YYYYMMDD-HHMM --update-file updates.json
gtdkit email journal event email-YYYYMMDD-HHMM gmail_marked_read --message-id mid-1 --increment marked_read
gtdkit email journal batch email-YYYYMMDD-HHMM --batch-file journal.json
gtdkit completions zsh > ~/.local/share/zsh/site-functions/_gtdkit
```

## Email Sessions

Email sessions use a dated directory containing:

```text
/Users/xeed/Library/Mobile Documents/com~apple~CloudDocs/SOPs/email-inbox-processing/YYYY/MM/DD/email-YYYYMMDD-HHMM/
```

`session init` prints the session ID. Later session-scoped commands accept that
ID and resolve it under the default root. Use `--root` to point at another SOP
root when testing or deliberately working elsewhere.

- `manifest.json` for session metadata and workflow contract paths.
- `queue.json` for inbox message metadata and processing state.
- `stats.json` for counters.
- `events.jsonl` for the append-only audit journal.
- `context.md`, `dashboards.md`, and `checkpoint.md` for human-readable resume
  state.

Session operations acquire `.session.lock`. JSON writes are staged through a
temporary file, flushed, and renamed into place so a failed command does not
leave partially written state.

## Development

The project is a single binary crate for now. Keep domain logic in pure
transforms where practical, and keep filesystem effects at the command/store
edge.

The code style favors practical functional Rust:

- Model workflow state with typed data structures.
- Keep validation, normalization, and queue/stat/event transforms pure where
  practical.
- Prefer immutable values and explicit return values in domain code.
- Use mutation deliberately at the filesystem boundary, for builders, and where
  Rust ownership makes it the clearest implementation.
- Use focused dependencies for CLI polish, diagnostics, locking, paths,
  serialization, and tests instead of hand-rolling infrastructure.

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

CI runs formatting, clippy, and tests on every pull request.
