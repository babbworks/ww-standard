# lib/ledger-config.sh

**Type:** Sourced bash library  
**Source:** `lib/ledger-config.sh`  
**Guard:** None (idempotent by convention)

---

## Role

Single owner of the `ledgers.yaml` contract. Manages the per-profile registry of named ledgers — resolving names to journal file paths, handling the string-or-map ambiguity, and enforcing schema rules.

---

## Schema

```yaml
ledgers:
  default: /abs/path/to.journal        # string form (path only)
  business:                            # map form (when metadata present)
    journal: /abs/path/to.journal
    commodity: USD
    hidden: false
```

Entry value is EITHER a string (absolute path) OR a map with path under `journal:`. Legacy `path:` key is READ (with deprecation warning) but NEVER written.

**Blessed map fields (v1):** `journal` (required in map form), `commodity`, `hidden`, `role`, `description`.

---

## Public Functions — Readers

| Function | Signature | Purpose |
|----------|-----------|---------|
| `ledger_cfg_path` | `()` | Return path to `ledgers.yaml` |
| `ledger_cfg_names` | `([--include-hidden])` | List all registered ledger names (excludes `default` key) |
| `ledger_cfg_resolve` | `(name)` | Resolve name → absolute journal path |
| `ledger_cfg_default_path` | `()` | Resolve the `default` entry |
| `ledger_cfg_get_field` | `(name, field)` | Read a metadata field |
| `ledger_cfg_exists` | `(name)` | Return 0 if ledger name exists |

---

## Public Functions — Writers

| Function | Signature | Purpose |
|----------|-----------|---------|
| `ledger_cfg_add` | `(name, path, [--commodity V] [--hidden] [--role R] [--desc D])` | Register a new ledger |
| `ledger_cfg_remove` | `(name)` | Deregister a ledger (cannot remove `default` or current default) |
| `ledger_cfg_rename` | `(old, new)` | Rename a ledger key (order-preserving) |
| `ledger_cfg_set_default` | `(name)` | Set the default ledger |
| `ledger_cfg_set_field` | `(name, field, value)` | Set a metadata field (upgrades string→map on write) |

All writes are atomic (tmp file + `os.replace`).

---

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | User/validation error (invalid name, already exists, etc.) |
| 2 | Not found |
| 3 | System error (PyYAML missing, parse error) |

---

## Constraints

- All YAML parsing via `python3`/`yaml.safe_load` — line-oriented shell parsers cannot survive the string-or-map ambiguity
- `default` is a reserved key: cannot be renamed or removed
- Ledger names must match `^[a-zA-Z0-9_-]+$`
- No `set -euo pipefail`, no `exit` — sourced library, return codes only
- A malformed `ledgers.yaml` is a fatal parse error, never a silent default

---

## Dependencies

- `python3` with `PyYAML`
- `$WORKWARRIOR_BASE` must be set (per-profile file)

---

## Changelog

- 2026-07-11 — Extracted as single owner (ledgers.yaml audit)
