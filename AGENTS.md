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
- Document non-obvious behavior with useful Rustdoc or comments as part of the
  implementation, not as an afterthought. Prioritize comments that explain
  invariants, mutation boundaries, validation-before-write guarantees, external
  workflow contracts, and why a command exists. Avoid comments that merely
  restate the next line of code.
- Tests should cover the behavior contract: queue validation, no partial
  mutation on failure, journal normalization, stats increments, and stable CLI
  output where useful.
- `docs/cli-reference.md` is generated from Clap metadata. When changing CLI
  commands, flags, status values, file schemas, session/journal behavior, or
  workflow command semantics, regenerate it with:

```sh
cargo run -- docs cli-reference --format markdown > docs/cli-reference.md
```

- Include the regenerated CLI reference in the same change. Tests should fail if
  it is stale.

## Feature QA

Every implemented feature, workflow change, CLI behavior change, or documentation
contract change requires an independent QA loop before the work is considered
done. This is a gate, not a nice-to-have.

### QA Workspace

Create one ignored QA workspace per feature:

```text
.agent/qa/YYYY-MM-DD-feature-name/
```

This path is intentionally ignored by Git. Do not commit QA reports, scratch
state, temp roots, screenshots, captured outputs, or other QA artifacts.

Each QA round must write a Markdown report in that directory:

```text
.agent/qa/YYYY-MM-DD-feature-name/round-N.md
```

### QA Loop

The implementer owns this loop:

1. Finish the candidate implementation and local documentation updates.
2. Run the relevant automated checks locally.
3. If QA must test the installed command, install the candidate binary first.
4. Spawn an independent QA subagent.
5. Require QA to use the actual installed or built binary, not mocks or cargo
   shortcuts, unless the behavior under test is library-only.
6. Require QA to test realistic user workflows, edge cases, failure paths, and
   attempts to break the feature.
7. Require QA to review documentation, generated references, SOPs, examples,
   and AGENTS guidance against actual behavior.
8. If QA reports findings, fix all of them.
9. Rerun the relevant automated checks and reinstall the binary when needed.
10. Spawn a new QA round with a new `round-N.md` report.
11. Repeat until QA reports `No findings`.

The task is not complete when the implementer fixes the last known issue. The
task is complete only after a follow-up QA round reports `No findings`.

### QA Findings

QA findings include:

- Behavior bugs or regressions.
- Missing edge-case handling.
- Bad CLI ergonomics or confusing errors.
- Incomplete tests for the changed contract.
- Documentation, generated-reference, SOP, example, or AGENTS mismatches.
- Missing instructions that would cause a future agent to repeat the same
  mistake.

Treat documentation and SOP findings as seriously as code findings. A feature
whose behavior is correct but whose docs are stale is not done.

### QA Report Template

Each QA report must be a Markdown file using this shape:

```markdown
# QA: <Feature Name> Round <N>

Date: YYYY-MM-DD
Commit under test: <short-sha or working tree>
Binary under test: <path from command -v, version output>
QA workspace: .agent/qa/YYYY-MM-DD-feature-name/

## Scope

- Code paths reviewed:
- User workflows tested:
- Documentation reviewed:

## Constraints

- Confirm no external state was mutated unless explicitly approved.
- Confirm temp roots / fixtures used.
- Confirm QA artifacts stayed under .agent/qa/YYYY-MM-DD-feature-name/.

## Commands Run

```sh
# exact commands, enough for reproduction
```

## Results

- PASS/FAIL for each realistic scenario.
- PASS/FAIL for edge cases and failure paths.
- PASS/FAIL for generated docs / SOP / AGENTS alignment.

## Findings

### Finding 1: <severity> - <title>

- Severity: High | Medium | Low
- File/line:
- Evidence:
- Expected:
- Actual:
- Suggested fix:

If there are no findings, write exactly:

No findings.

## Verdict

- Findings remain: yes/no
- QA recommends another round: yes/no
```

The final accepted QA report for a feature must contain `No findings` and
`Findings remain: no`.

## Commands

Use the managed Rust toolchain:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

For local install checks:

```sh
cargo install --path . --locked --force
gtdkit --version
```
