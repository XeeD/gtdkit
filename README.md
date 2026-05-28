# gtdkit

`gtdkit` is a Rust CLI for local GTD workflow state. The first shipped domain is
email inbox-processing session state, replacing the older Python helper scripts
while preserving their JSON and Markdown file formats.

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

Session files are locked with `.session.lock`. JSON writes are staged through a
temporary file, flushed, and renamed into place.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```
