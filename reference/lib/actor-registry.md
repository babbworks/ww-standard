# lib/actor-registry.sh

**Type:** Sourced bash library  
**Source:** `lib/actor-registry.sh`  
**Guard:** `[[ -n "${_WW_ACTOR_REGISTRY_LOADED:-}" ]]` — safe to source multiple times

---

## Role

Single owner of `config/actors.yaml`. No other code path may read or write that file. Manages actor identity at the instance level with profile-level participation overlays. Identity resolves at instance tier only — a profile MUST NOT define an actor the instance does not know.

Schema: `dev/schemas/actors-v1.md`

---

## Resolution Model

- **Instance** (`config/actors.yaml`): defines all known actors (id, kind, display, status, source)
- **Profile** (`$WORKWARRIOR_BASE/actors.yaml`): overlay defining which actors participate (members) and their weights

A profile references instance actors but cannot create new ones.

---

## Public Functions — Readers

| Function | Signature | Purpose |
|----------|-----------|---------|
| `actor_reg_list` | `()` | List all actor IDs (one per line) |
| `actor_reg_get` | `(id)` | Print all fields as `key=value` lines |
| `actor_reg_exists` | `(id)` | Return 0 if actor exists, 1 if not |
| `actor_reg_kind` | `(id)` | Print the kind field |
| `actor_reg_display` | `(id)` | Print the display name |
| `actor_reg_members` | `()` | List actors participating in the active profile |
| `actor_reg_weight` | `(id)` | Print weight for actor in profile (default 1.0) |

---

## Public Functions — Writers

| Function | Signature | Purpose |
|----------|-----------|---------|
| `actor_reg_add` | `(id, display, [source])` | Add new actor. Refuses non-`local` source. Appends nano suffix per id_scheme. |
| `actor_reg_set_status` | `(id, active\|inactive)` | Change status. NO DELETE. |
| `actor_reg_set_field` | `(id, field, value)` | Set arbitrary field. Refuses `kind` (immutable). |

---

## ID Scheme

Controlled by `id_scheme` in `config/actors.yaml`:

| Scheme | Suffix | Example |
|--------|--------|---------|
| `solo` | none | `human:morgen` |
| `team` | 4-char base36 nano | `human:morgen.a7x2` |
| `enterprise` | 6-char base36 nano | `human:morgen.k9m4p2` |

Collision-checked: retries up to 100 times if the generated ID already exists.

---

## Constraints

- All YAML parsing via `python3`/`yaml.safe_load`
- Unknown fields are PRESERVED on rewrite (round-trip safe)
- Source must be `local` for adds (no remote actor creation)
- `kind` is immutable — encoded in the actor ID prefix
- Actors are soft-deleted (status → inactive), never removed

---

## Dependencies

- `services/stream/lib/actor.sh` — for `actor_validate()`
- `python3` with `PyYAML`

---

## Changelog

- 2026-07-12 — Initial implementation (TASK-STRM-002)
