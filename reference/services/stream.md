# services/stream — Workwarrior Stream Service (WWSS)

**Type:** Service script (executable)  
**Source:** `services/stream/stream.sh`  
**Dependencies:** `lib/core-utils.sh`, `lib/actor-registry.sh`, `services/stream/lib/*`

---

## Role

Append-only temporal event log with pluggable lens projections. The stream is the single source of truth for all time-tracking and activity events. Every redundant store (journals, reports, dashboards) is a pure function of this log.

Substrate: Pacioli (append-only) + Hollerith (positional encoding).

---

## Event Format

```
<unix_ts> <OP> <action> <object> <ctx_json>
```

Five positional fields. The encoding is **frozen** — consumers split on whitespace with limit 5, so `ctx_json` is the rest of the line. New dimensions go inside `ctx_json`, never as additional fields.

### Op Codes

| Code | Meaning | Source |
|------|---------|--------|
| `T` | Task event | TaskWarrior hooks, ingest |
| `F` | Frick — state transition (task active/start) | TaskWarrior hooks |
| `B` | Bundy — punch clock interval (start/stop) | Punch commands, timew hook |
| `D` | Dey — behavioral signal | Derived by lenses |
| `H` | Hollerith — positional header (legacy) | System |
| `S` | System — schema, reset | System events |
| `A` | Annotation — journal write | jrnl ingest |

### Schema Versioning

The schema version is an **event**, not a header. An `S schema v1` event stamps the log. Absence of any schema event means v0. The log has no header line (a comment would be garbage to the positional parser).

---

## Subcommands

### `ww stream emit <op> <action> <object> [ctx]`

Append one event to stream.log. Accepts `--actor <id>` to inject actor into ctx.

### `ww stream punch in|out`

```
ww stream punch in|out --actor <id> --object <obj> [--proj P] [--tags t1 t2]
                       [--at <time>] [--reason <text>] [--by <reconciler>]
```

Emit a `B start` or `B stop` event. Validates actor against the actor registry. `--reason` is only valid on punch out (reconciled close). If the object is already closed, punch out is a no-op.

### `ww stream board`

```
ww stream board [--actor <filter>] [--proj <filter>] [--stale <seconds>] [--format text|json]
```

Show who/what is currently punched in. Computes the open set — `B start` events with no matching `B stop`. Read-only.

### `ww stream reconcile`

```
ww stream reconcile --stale <seconds> --reason <text> [--at <time>] [--confirm]
```

Close stale punches. Without `--confirm`: dry run (shows what would be closed). With `--confirm`: appends reconciled `B stop` events. Each closed event carries a `reconciled` key in ctx with `by`, `at`, and `reason` fields.

### `ww stream ingest [--source SOURCE] [--from DATE]`

Ingest from WW data sources. SOURCE: `tasks|timew|jrnl|ledger|all` (default: all). Deduplicates against existing events before appending.

### `ww stream view [--lens NAME] [--format FMT] [--from DATE] [--to DATE]`

Project stream through a lens. Alias: `replay`. Default lens: `burroughs`. Formats: `text|json|ascii`.

### `ww stream sessions [--gap SECS] [--from DATE] [--to DATE]`

Detect and display session boundaries based on inactivity gaps. Default gap: 300 seconds.

### `ww stream lens list`

List all available lenses with descriptions.

### `ww stream hooks install|remove|status [--profile NAME]`

Manage task hook scripts that emit stream events on TaskWarrior task add/modify.

### `ww stream status`

Show log stats: event count, last event, active profile, schema version.

### `ww stream reset --confirm`

Truncate stream.log. Destructive — requires `--confirm` flag.

---

## Lenses

Lenses are pluggable projection scripts in `services/stream/lenses/`. Each implements `lens_describe()` and `lens_run()`.

| Lens | File | Purpose |
|------|------|---------|
| **board** | `board.sh` | The board — who/what is currently punched in (open set) |
| **burroughs** | `burroughs.sh` | Raw chronological event log |
| **bundy** | `bundy.sh` | Interval accumulation — duration totals with ASCII timeline |
| **hollerith** | `hollerith.sh` | Matrix grid: time-bucket rows × object columns |
| **pacioli** | `pacioli.sh` | Running event balance per object (ledger view) |
| **frick** | `frick.sh` | State transitions — F op code timeline per object |
| **felt** | `felt.sh` | Activity density — event-count heat map across time buckets |
| **dey** | `dey.sh` | Behavioral signal — intensity/stability/fragmentation time-series |
| **cooper** | `cooper.sh` | Cooper field — geometric polar projection of Dey signal |

---

## Internal Libraries (`services/stream/lib/`)

### `actor.sh`

Single owner of actor identity resolution. Defines actor kinds (`human|agent|machine`), the v0 fallback rule, and validation.

| Function | Signature | Purpose |
|----------|-----------|---------|
| `actor_resolve` | `(ctx_json)` | Resolve actor from ctx; applies v0 fallback (`profile:<prof>`) |
| `actor_validate` | `(id)` | Validate `<kind>:<local-id>` format |
| `actor_kind` | `(id)` | Extract kind prefix |
| `actor_local` | `(id)` | Extract local-id suffix |
| `actor_is_fallback` | `(id)` | True if this is a v0 fallback attribution |

### `adapters.sh`

Ingest adapters that transform external data into stream events.

| Function | Purpose |
|----------|---------|
| `adapt_tasks` | Export TaskWarrior tasks → `T` events |
| `adapt_timew` | Export TimeWarrior intervals → `B start/stop` events |
| `adapt_jrnl` | Export jrnl entries → `A write` events |
| `adapt_ledger` | Export hledger transactions → `T post` events |
| `_dedup_events` | Filter events already present in the log |

### `replay.sh`

Stream loading and lens dispatch.

| Function | Signature | Purpose |
|----------|-----------|---------|
| `replay_load` | `([from_ts] [to_ts])` | Load events, excluding `H` and `S` system ops |
| `replay_apply_lens` | `(name)` | Source and execute a lens script |

### `codecs.sh`

Output format encoders.

| Function | Purpose |
|----------|---------|
| `codec_json` | Convert events to JSON array |
| `codec_text` | Pass-through (identity) |
| `codec_ascii` | Pass-through (identity) |

### `lenses.sh`

Lens discovery and listing.

| Function | Purpose |
|----------|---------|
| `lens_list` | Enumerate available lenses with descriptions |

### `taoo.sh`

TAOO (Time-Action-Object-Occasion) classification helper.

| Function | Purpose |
|----------|---------|
| `taoo_classify` | Classify an event into TAOO fields |
| `taoo_filter` | Filter events by pattern on object field |

---

## Python Scripts

### `attention.py`

The attention queue: ranks accounts by weighted value using the actor coefficient vector.

```
python3 attention.py --ww-base <WW_BASE> --profile-base <WORKWARRIOR_BASE>
                     [--top N] [--format text|json] [--filter <str>]
                     [--show-weights] [--at <date>] [--weights-file <path>]
```

Mechanism: reads actor weights from `config/actors.yaml`, runs `hledger balance` against the derived stream journal, multiplies each actor's hours by their weight, and ranks accounts by attention value. The coefficients are data — dated, diffable, inspectable.

### `sync-to-journal.py`

Derive hledger journal from stream.log punch intervals. Output is a **pure function** of stream.log — running twice produces byte-identical output.

```
python3 sync-to-journal.py --stream-log <path> --ww-base <WW_BASE>
                           [--output <path>] [--dry-run] [--generated-ts <ts>]
```

Generates double-entry postings: `time:project:<proj>` (debit, hours received) and `actors:<actor-id>` (credit, hours given).

---

## Hooks

### `hooks/timew-stream-hook.sh`

Sourced helper (not standalone) that emits `B start/stop` events when `timew start/stop` succeed. Called from the `timew()` wrapper in `lib/shell-integration.sh`.

Contract:
- Object hashes MUST match `adapt_timew` (sha256 of start_str + tags)
- Fail-soft: never breaks timew (returns 0 on any error)
- No `set -euo pipefail`, no `exit`

Key functions:
- `_stream_timew_emit_start(timewarriordb, profile)` — emit after successful `timew start`
- `_stream_timew_emit_stop(timewarriordb, profile)` — emit after successful `timew stop`
- `_stream_timew_hook_installed()` — check if hook is active

---

## Design Principles

1. **The stream is the clock.** TimeWarrior is a card reader (input device). The derived journal is a cache.
2. **Append-only.** Events are never rewritten. Even schema version and reset are events.
3. **Every redundant store is a pure function of the log.** Journals, reports, and dashboards are derived — never authoritative.
4. **The capture→record→interpret pipeline.** Capture (timew, task hooks) → Record (stream.log) → Interpret (lenses, sync-to-journal).

---

## Changelog

- 2026-07-15 — Punch, reconcile, board, actor axis (v1 schema)
- 2026-07-09 — Initial stream service with lenses
