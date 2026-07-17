# lib/instance-registry.sh

**Type:** Sourced bash library  
**Source:** `lib/instance-registry.sh`  
**Guard:** None (idempotent by convention)

---

## Role

Directory-manifest-based instance registry. Manages multiple workwarrior installations (instances) via JSON manifests stored in `~/.config/ww/registry/`. Enables a warrior spanning instances to answer "where are my installations and what state are they in?"

---

## Storage

- Registry dir: `~/.config/ww/registry/` (overridable via `WW_REGISTRY_DIR`)
- Manifest format: `<instance-id>.json`
- Last instance: `~/.config/ww/last-instance`

### Manifest Schema

```json
{
  "id": "my-instance",
  "alias": "my-instance",
  "version": "1.0.0",
  "visibility": "visible",
  "install_path": "/home/user/ww",
  "preset": "multi",
  "command_name": "ww",
  "security_backend": "auto",
  "lock_required": false,
  "parent_anchor": null,
  "allowed_orchestrators": [],
  "registered_at": "2026-07-10T12:00:00Z",
  "status": "active"
}
```

---

## Public Functions

| Function | Signature | Purpose |
|----------|-----------|---------|
| `ww_config_home` | `()` | Return config home (`~/.config/ww`) |
| `ww_registry_dir` | `()` | Return registry directory |
| `ww_registry_init` | `()` | Create registry directory |
| `ww_manifest_path` | `(iid)` | Return manifest file path for instance |
| `ww_instance_register` | `(iid, install_path, [visibility], [alias], [preset], [command_name], [backend], [parent_anchor])` | Register a new instance |
| `ww_instance_set_visibility` | `(iid, visible\|hidden)` | Change instance visibility |
| `ww_instance_detach` | `(iid)` | Remove instance manifest (hard delete) |
| `ww_instance_list` | `([include_hidden])` | List instances (TSV: id, alias, version, visibility, status) |
| `ww_instance_where` | `(iid)` | Return install_path for instance |
| `ww_instance_lookup` | `(key, [include_hidden])` | Find instance by id or alias |
| `ww_set_last_instance` | `(iid)` | Record last active instance |
| `ww_get_last_instance` | `()` | Read last active instance |
| `ww_instance_lock_required` | `(iid)` | Return "true" or "false" for lock requirement |

---

## Presets

| Preset | Lock Required | Notes |
|--------|---------------|-------|
| `multi` | false | Default — multiple parallel instances |
| `hardened` | true | Security-focused — requires lock acquisition |

---

## Constraints

- Instance manifests are JSON (written via heredoc, read via `python3`)
- Visibility filtering: hidden instances excluded from list/lookup unless `include_hidden=1`
- `parent_anchor` and `allowed_orchestrators` support hierarchical instance control
- No YAML dependency — uses JSON only

---

## Dependencies

- `python3` — JSON reading/writing
- Filesystem: `~/.config/ww/registry/`

---

## Changelog

- 2026-07-10 — Initial implementation
