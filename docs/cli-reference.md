# gtdkit CLI Reference

Generated with `gtdkit docs cli-reference --format markdown`.

## `gtdkit`

GTD workflow utilities

Subcommands:

- `docs`
- `email`
- `completions`

### `gtdkit docs`

Generate checked-in documentation artifacts

Subcommands:

- `cli-reference`

#### `gtdkit docs cli-reference`

Generate the Markdown CLI reference from Clap metadata

| Argument | Required | Repeatable | Help |
| --- | --- | --- | --- |
| `--format` | false | false | Output format |

### `gtdkit email`

Manage local email inbox-processing session state

Subcommands:

- `session`
- `queue`
- `journal`
- `research`
- `step`
- `action`
- `fresh-check`

#### `gtdkit email session`

Create or apply durable session-level updates

Subcommands:

- `init`
- `apply`

##### `gtdkit email session init`

Create a dated restartable email session

| Argument | Required | Repeatable | Help |
| --- | --- | --- | --- |
| `<session_id>` | false | false | Optional short session ID |
| `--root` | false | false | Email SOP root directory |
| `--gmail-query` | false | false | Gmail query recorded in manifest |
| `--account` | false | false | Account recorded in manifest |
| `--timezone` | false | false | Local timezone label |
| `--allow-active-session` | false | false | Allow another active same-day session |

##### `gtdkit email session apply`

Apply a low-level multi-file local session update

| Argument | Required | Repeatable | Help |
| --- | --- | --- | --- |
| `<session_id>` | true | false | Short session ID |
| `--root` | false | false | Email SOP root directory |
| `--batch-file` | false | false | JSON batch file to apply |
| `--event` | false | true | Event JSON object; repeatable |
| `--queue-update` | false | true | Queue update JSON object; repeatable |
| `--stat-increment` | false | true | Stats key to increment; repeatable |
| `--context-append` | false | true | Markdown context line; repeatable |
| `--dashboard-append` | false | false | Markdown dashboard text to append |
| `--checkpoint-write` | false | false | Checkpoint Markdown replacement |
| `--timezone` | false | false | Local timezone label |

#### `gtdkit email queue`

Build, inspect, or update the inbox queue

Subcommands:

- `build`
- `view`
- `update`

##### `gtdkit email queue build`

Create or extend queue entries from supplied Gmail metadata

| Argument | Required | Repeatable | Help |
| --- | --- | --- | --- |
| `<session_id>` | true | false | Short session ID |
| `--root` | false | false | Email SOP root directory |
| `--items-file` | false | false | JSON file containing queue items |
| `--item` | false | true | Queue item JSON object; repeatable |
| `--replace` | false | false | Replace the existing queue |
| `--timezone` | false | false | Local timezone label |

##### `gtdkit email queue view`

Inspect the current queue through the session lock

| Argument | Required | Repeatable | Help |
| --- | --- | --- | --- |
| `<session_id>` | true | false | Short session ID |
| `--root` | false | false | Email SOP root directory |
| `--status` | false | false | Filter by queue status |
| `--limit` | false | false | Maximum number of items to show |
| `--json` | false | false | Emit JSON instead of a table |

##### `gtdkit email queue update`

Apply queue-only field updates from JSON

| Argument | Required | Repeatable | Help |
| --- | --- | --- | --- |
| `<session_id>` | true | false | Short session ID |
| `--root` | false | false | Email SOP root directory |
| `--update-file` | true | false | JSON update payload |
| `--timezone` | false | false | Local timezone label |

#### `gtdkit email journal`

Append journal events and compatibility batches

Subcommands:

- `event`
- `batch`

##### `gtdkit email journal event`

Append one journal event and optional queue/stat updates

| Argument | Required | Repeatable | Help |
| --- | --- | --- | --- |
| `<session_id>` | true | false | Short session ID |
| `<event>` | false | false | Event name |
| `--root` | false | false | Email SOP root directory |
| `--event` | false | false | Event name alternative to positional |
| `--message-id` | false | false | Queue message ID associated with the event |
| `--data` | false | false | Event data JSON object |
| `--increment` | false | true | Stats key to increment; repeatable |
| `--set-status` | false | false | Set queue status |
| `--set-approval-state` | false | false | Set queue approval_state |
| `--set-research-state` | false | false | Set queue research_state |
| `--set-read-state` | false | false | Set queue read_state |
| `--set-recommended-action` | false | false | Set queue recommended_action |
| `--set-terminal-action` | false | false | Set queue terminal_action |
| `--set-dashboard-anchor` | false | false | Set queue dashboard_anchor |
| `--timezone` | false | false | Local timezone label |

##### `gtdkit email journal batch`

Append a compatibility batch of journal events

| Argument | Required | Repeatable | Help |
| --- | --- | --- | --- |
| `<session_id>` | true | false | Short session ID |
| `--root` | false | false | Email SOP root directory |
| `--batch-file` | true | false | JSON event batch file |
| `--timezone` | false | false | Local timezone label |

#### `gtdkit email research`

Record read-only research-agent output

Subcommands:

- `digest`

##### `gtdkit email research digest`

Record a read-only subagent digest

| Argument | Required | Repeatable | Help |
| --- | --- | --- | --- |
| `<session_id>` | true | false | Short session ID |
| `--root` | false | false | Email SOP root directory |
| `--message-id` | true | false | Queue message ID researched |
| `--queue-index` | false | false | Queue index researched |
| `--agent-id` | false | false | Research agent identifier |
| `--agent-name` | false | false | Research agent display name |
| `--recommended-action` | false | false | Recommended action from digest |
| `--no-mutations-performed` | false | false | Assert the subagent performed no mutations |
| `--state` | false | false | Queue research_state to store |
| `--digest` | false | false | Digest text or compact JSON |
| `--thread-id` | false | false | Backfill Gmail thread ID from the read-only research result |
| `--internal-date` | false | false | Backfill message internal date from the read-only research result |
| `--from` | false | false | Backfill message From header |
| `--subject` | false | false | Backfill message subject |
| `--snippet` | false | false | Backfill message snippet |
| `--label` | false | true | Backfill Gmail label; repeatable |
| `--timezone` | false | false | Local timezone label |

#### `gtdkit email step`

Record dashboard workflow steps

Subcommands:

- `dashboard`

##### `gtdkit email step dashboard`

Record a rendered dashboard and approval request

| Argument | Required | Repeatable | Help |
| --- | --- | --- | --- |
| `<session_id>` | true | false | Short session ID |
| `--root` | false | false | Email SOP root directory |
| `--message-id` | true | false | Queue message ID dashboarded |
| `--dashboard-anchor` | true | false | Stable dashboard anchor stored in session metadata |
| `--recommended-action` | false | false | Recommended action shown to the user |
| `--approval-options` | false | true | Comma-separated approval options |
| `--read-state` | false | false | Queue read_state to store |
| `--dashboard-stdin` | false | false | Read dashboard Markdown from stdin |
| `--dashboard-text` | false | false | Dashboard Markdown text |
| `--email-started` | false | false | Also journal email_started |
| `--timezone` | false | false | Local timezone label |

#### `gtdkit email action`

Record action approval and completion

Subcommands:

- `approve`
- `complete`

##### `gtdkit email action approve`

Record explicit user approval for an action

| Argument | Required | Repeatable | Help |
| --- | --- | --- | --- |
| `<session_id>` | true | false | Short session ID |
| `--root` | false | false | Email SOP root directory |
| `--message-id` | true | false | Queue message ID approved |
| `--action` | true | false | Approved action |
| `--user-reply` | false | false | User confirmation text |
| `--timezone` | false | false | Local timezone label |

##### `gtdkit email action complete`

Record a completed terminal action after verification

| Argument | Required | Repeatable | Help |
| --- | --- | --- | --- |
| `<session_id>` | true | false | Short session ID |
| `--root` | false | false | Email SOP root directory |
| `--message-id` | true | false | Queue message ID completed |
| `--terminal-action` | true | false | Terminal queue status/action |
| `--gmail-action` | false | false | External Gmail action recorded as metadata |
| `--stat` | false | true | Stats key to increment; repeatable |
| `--verification` | false | false | Verification note |
| `--timezone` | false | false | Local timezone label |

#### `gtdkit email fresh-check`

Record a fresh Gmail inbox check result

| Argument | Required | Repeatable | Help |
| --- | --- | --- | --- |
| `<session_id>` | true | false | Short session ID |
| `--root` | false | false | Email SOP root directory |
| `--count` | true | false | Fresh inbox message count |
| `--message-id` | false | true | Fresh message ID; repeatable |
| `--timezone` | false | false | Local timezone label |

### `gtdkit completions`

Generate shell completions

| Argument | Required | Repeatable | Help |
| --- | --- | --- | --- |
| `<shell>` | true | false | Shell to generate completions for |

