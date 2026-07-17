# lib/deps-state.sh

**Type:** Sourced bash library  
**Source:** `lib/deps-state.sh`  
**Guard:** `[[ -n "${_WW_DEPS_STATE_LOADED:-}" ]]` — safe to source multiple times

---

## Role

The toolchain state stamp — the `apt update` model. Probes tool versions once (on `ww deps check`), writes a JSON stamp, and provides a cheap runtime guard that reads the stamp without forking subprocesses.

Solves: the runtime floor guard originally probed `hledger --version` on every call. Since `ww` is a fresh process each invocation, exported flags never persisted. This stamp replaces per-invocation probes with a single file read.

---

## State File

Path: `$WW_BASE/.state/deps.json` (per-instance, gitignored)

```json
{
  "schema": "v1",
  "checked_at": 1720000000,
  "manifest": "<sha256 of config/dependencies.yaml>",
  "tools": {
    "hledger": {"version": "1.40", "min": "1.34", "status": "ok", "path": "/usr/local/bin/hledger"},
    "timew":   {"version": "1.7.1", "min": "1.4.0", "status": "ok", "path": "/usr/bin/timew"}
  }
}
```

The stamp is invalidated when the manifest changes — a raised floor in `config/dependencies.yaml` won't leave hosts stamped "ok" against the old one.

---

## Public Functions

| Function | Signature | Purpose |
|----------|-----------|---------|
| `deps_state_path` | `()` | Return path to the state stamp file |
| `deps_state_refresh` | `()` | Probe all tools and write the stamp (FORKS — slow path) |
| `deps_state_fresh` | `()` | Return 0 if stamp exists and matches current manifest |
| `deps_state_get` | `(tool, field)` | Read a field from the stamp (no fork) |
| `deps_require_floor` | `(tool)` | The runtime guard — cheap read, self-heals if stale |

---

## `deps_require_floor(tool)` — The Runtime Guard

1. Advisory tools never block (returns 0)
2. Checks if stamp is fresh; self-heals (one probe) if not
3. Reads status from stamp
4. `ok` → returns 0
5. `below_floor` or `missing` → prints diagnostic error, returns 2

Fails **loudly** on a hard floor. Never silently degrades: an hledger below the floor doesn't error on `--value`, it returns unconverted numbers — a wrong answer with no error is the worst outcome.

---

## Constraints

- Sourced library: no `set -euo pipefail`, no `exit`
- The ONLY place version probes are allowed on a non-hot path
- Self-healing: stamp absent → refresh once, then read
- Falls back to live probe if stamp is unreadable even after refresh

---

## Dependencies

- `lib/deps-manifest.sh` — for tool list, version floors, enforcement level
- `python3` — JSON stamp writing
- `sha256sum` — manifest fingerprinting

---

## Changelog

- 2026-07-13 — Initial implementation (TASK-DEPS-001)
