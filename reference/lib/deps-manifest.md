# lib/deps-manifest.sh

**Type:** Sourced bash library  
**Source:** `lib/deps-manifest.sh`  
**Guard:** `[[ -n "${_WW_DEPS_MANIFEST_LOADED:-}" ]]` — safe to source multiple times

---

## Role

Single owner of `config/dependencies.yaml`. No other code path may parse that file. Provides all queries against the dependency manifest: version floors, pinned versions, platform assets, package managers, installation strategy, and the static binary installer.

---

## Public Functions — Tool Facts

| Function | Signature | Purpose |
|----------|-----------|---------|
| `deps_manifest_path` | `()` | Resolve path to `config/dependencies.yaml` |
| `deps_tools` | `()` | List all tool names (newline-separated) |
| `deps_min_version` | `(tool)` | Minimum required version |
| `deps_pinned` | `(tool)` | Pinned release version |
| `deps_strategy` | `(tool)` | Installation strategy (default: `package`) |
| `deps_display` | `(tool)` | Human-readable tool name |
| `deps_floor_reason` | `(tool)` | Why the floor exists |
| `deps_enforcement` | `(tool)` | `hard` or `advisory` — only `hard` refuses to run |
| `deps_is_hard` | `(tool)` | Return 0 if enforcement is hard |
| `deps_package` | `(tool, manager)` | Package name under a package manager (or `unsupported`) |

---

## Public Functions — Platform & Assets

| Function | Signature | Purpose |
|----------|-----------|---------|
| `deps_platform` | `()` | Canonical platform key (`linux-x64`, `mac-arm64`, etc.) |
| `deps_asset_file` | `(tool, platform)` | Archive filename for tool+platform |
| `deps_asset_sha256` | `(tool, platform)` | Expected checksum |
| `deps_asset_url` | `(tool, platform)` | Full download URL (template expanded) |
| `deps_install_to` | `(tool)` | Target installation directory |

---

## Public Functions — Version Comparison

| Function | Signature | Purpose |
|----------|-----------|---------|
| `deps_version_gte` | `(have, want)` | Return 0 if `have >= want` (uses `sort -V`) |
| `deps_probe_version` | `(tool)` | Probe installed version (FORKS — hot path must use stamp) |

---

## Public Functions — Static Installer

### `deps_install_static(tool)`

Downloads, verifies, and installs a tool's official static binary.

Steps:
1. Resolve platform (fails on unknown)
2. Check that upstream builds for this platform (fails if `unsupported`)
3. Download via `curl`
4. **Verify SHA-256 checksum BEFORE making executable** (refuse on mismatch)
5. Extract and install to target directory
6. Verify installed binary reports expected version
7. Warn if PATH ordering shadows the new binary

---

## Constraints

- A malformed manifest is FATAL, never a silent default
- All queries via `python3`/`yaml.safe_load` (single internal function `_deps_query`)
- Version comparison uses `sort -V` (handles integer minors like hledger's 1.9 < 1.52)
- The platform matrix is DATA in the YAML — never shell literals
- `deps_probe_version` FORKS — callers on a hot path must use `lib/deps-state.sh`

---

## Dependencies

- `python3` with `PyYAML`
- `curl` — required for static downloads
- `sha256sum` — checksum verification
- `config/dependencies.yaml`

---

## Changelog

- 2026-07-13 — Initial implementation
