# lib/ledger-dispatch.sh

**Type:** Sourced bash library  
**Source:** `lib/ledger-dispatch.sh`  
**Guard:** None (idempotent by convention)

---

## Role

Shared dispatcher for `ww ledger` and the `l` shell function. A small set of reserved management words act on `ledgers.yaml`; everything else passes through to `hledger -f "$LEDGER_F"`. One owner, thin callers — the two surfaces cannot drift apart.

---

## Public Functions

### `ledger_dispatch(action, [args...])`

Main entry point. Accepts optional `-L|--ledger <name>` prefix to select a named ledger.

**Reserved management words:**

| Action | Purpose |
|--------|---------|
| `new\|create` | Create a new named ledger (registers in `ledgers.yaml`, creates `.journal` file) |
| `remove\|delete\|rm` | Remove a named ledger (deregisters and deletes file) |
| `rename` | Rename a ledger key (file unchanged; schema owner is source of truth for name→path) |
| `list` | List all registered ledger names |
| `set-default` | Set the default ledger for the profile |
| `inventory` | Run `scripts/build-ledger-inventory.sh` |
| `custom` | Launch interactive ledger configuration |
| `help` | Show help |
| `""` (bare) | Balance overview of default ledger |

**Everything else** passes through to `hledger` with the resolved ledger file injected via `-f`.

### `ledger_refuse_auto_writeback(content, [target_file])`

Safety gate: refuses to write content containing auto-expanded postings into the ledger tree. Prevents the double-count hazard where `--auto` output piped back causes rules to fire again on derived postings.

Returns 0 if writing is safe, 1 if refused.

---

## Internal Functions

| Function | Purpose |
|----------|---------|
| `_ledger_default_file()` | Resolve active profile's default ledger (0 forks when `LEDGER_F` is set) |
| `_ledger_passthrough(target, args...)` | Pass args to hledger with target file; guards version floor |
| `_ledger_target([name])` | Resolve passthrough target from optional ledger name |

---

## Constraints

- Sourced by both `bin/ww` and `lib/shell-integration.sh` `l()` function
- No `set -euo pipefail`, no `exit` — return codes only
- Depends on `ledger_cfg_*` functions from `lib/ledger-config.sh`
- Guards hledger version floor via `hledger_require_floor()` if available
- The `--auto` writeback refusal is belt-and-suspenders: both flag-based and content-smell-based

---

## Dependencies

- `lib/ledger-config.sh` — ledger registry operations
- `lib/core-utils.sh` — `log_error`, `log_success`
- `hledger` — external binary (version floor enforced)

---

## Changelog

- 2026-07-12 — Extracted from bin/ww with write-back refusal (TASK-LED-016)
