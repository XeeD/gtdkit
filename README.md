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
gtdkit email session apply <session-dir> --batch-file batch.json
gtdkit email queue build <session-dir> --items-file inbox-items.json --replace
gtdkit email queue view <session-dir> --status pending --json
gtdkit email queue update <session-dir> --update-file updates.json
gtdkit email journal event <session-dir> gmail_marked_read --message-id mid-1 --increment marked_read
gtdkit email journal batch <session-dir> --batch-file journal.json
gtdkit completions zsh > ~/.local/share/zsh/site-functions/_gtdkit
```

## Email Sessions

Email sessions use a dated directory containing:

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

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

CI runs formatting, clippy, and tests on every pull request.
