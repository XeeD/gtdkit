# AGENTS.md

## Project

`gtdkit` is a standalone Rust CLI for local GTD workflow state. Treat it as a
durable state tool: command handlers should validate inputs, apply predictable
domain transforms, and write session files safely.

## Engineering Preferences

- Prefer practical functional design: small pure functions, explicit data
  transforms, and mutation only where it makes the code clearer or avoids
  needless copying.
- Prefer immutability by default inside domain code. Use mutable state at the
  filesystem boundary, for builders, and when Rust ownership makes that the
  straightforward option.
- Be comfortable with focused dependencies. Use established crates for CLI UX,
  diagnostics, tables, completions, testing, paths, locking, and serialization
  instead of hand-rolling niceties.
- Keep command UX polished and modern rather than preserving incidental legacy
  invocation details.
- Preserve documented file formats unless intentionally versioning the schema.
- Validate complete operations before mutating files. A failed command should
  leave session state unchanged whenever feasible.
- Keep filesystem effects isolated. Use session-level locks and atomic writes
  for durable state changes.
- Tests should cover the behavior contract: queue validation, no partial
  mutation on failure, journal normalization, stats increments, and stable CLI
  output where useful.

## Commands

Use the managed Rust toolchain:

```sh
mise exec -- cargo fmt --check
mise exec -- cargo clippy --all-targets --all-features -- -D warnings
mise exec -- cargo test
```

For local install checks:

```sh
mise exec -- cargo install --path . --locked --force
gtdkit --version
```
