# lib/unit-registry.sh

**Type:** Sourced bash library  
**Source:** `lib/unit-registry.sh`  
**Guard:** `[[ -n "${_WW_UNIT_REGISTRY_LOADED:-}" ]]` — safe to source multiple times

---

## Role

Single owner of `config/units.yaml`. No other code path may parse that file. Manages units of measure, their dimensions, symbols, equivalences, and conversion to hledger price directives.

Schema: `dev/schemas/units-v1.md`

---

## Two Invariants

1. **An equivalence is same-dimension.** `1 kWh = 1000 Wh` is a CONVERSION (always true). "An hour is worth 50 USD" is a PRICE (contingent, contextual). Cross-dimension equivalences are REJECTED — the boundary between conversion and valuation cannot be crossed by accident.

2. **Authoritative, with a loud unknown.** An unregistered unit is REPORTED, never silently coerced or dropped. The log is truth; the registry is a lookup table. An unknown unit is a REGISTRY problem, not a data problem.

---

## Public Functions — Readers

| Function | Signature | Purpose |
|----------|-----------|---------|
| `unit_reg_list` | `()` | List all unit IDs |
| `unit_reg_exists` | `(id)` | Return 0 if unit exists |
| `unit_reg_display` | `(id)` | Human-readable display name |
| `unit_reg_dimension` | `(id)` | The unit's dimension (e.g., `time`, `energy`) |
| `unit_reg_precision` | `(id)` | Decimal precision (default 2) |
| `unit_reg_symbol` | `(id)` | The hledger commodity string |
| `unit_reg_by_symbol` | `(symbol)` | Resolve hledger symbol back to unit id |
| `unit_reg_equivalences` | `(id)` | List equivalences as `<to> <factor>` lines |
| `unit_reg_dimension_base` | `(dimension)` | Return the base unit for a dimension |
| `unit_reg_symbol_known` | `(symbol)` | Is this symbol registered? |
| `unit_reg_unregistered` | `()` | (stdin) Print symbols the registry has never seen |

---

## Public Functions — Validation

| Function | Signature | Purpose |
|----------|-----------|---------|
| `unit_reg_validate` | `()` | Validate entire registry. Returns 0 if coherent. |
| `unit_reg_price_directives` | `([date])` | Generate hledger `P` directives from equivalences |

Validation checks:
- Per-unit: required fields, valid equivalence targets, same-dimension constraint, positive factors
- Per-dimension: exactly one base unit (`base: true`)
- Connectivity: every unit reachable from the dimension base via BFS

---

## Public Functions — Writers

| Function | Signature | Purpose |
|----------|-----------|---------|
| `unit_reg_add` | `(id, display, dimension, symbol, precision, [equivalences])` | Add a new unit. Validates, then reverts on failure. |

Equivalences format: comma-separated `"to:factor"` pairs (e.g., `"Wh:1000,MWh:0.001"`).

---

## hledger Integration

The registry generates `P DATE <symbol> <factor> <to-symbol>` price directives. hledger's price engine acts as a unit-conversion algebra: `P DATE kWh 1000 Wh` plus `-X Wh` converts `3 kWh` → `3000 Wh`.

---

## Constraints

- A malformed registry is FATAL, never a silent default
- All YAML parsing via `python3`/`yaml.safe_load`
- Guessing what a quantity means (e.g., adding kilograms to hours) is prevented at the registry level

---

## Dependencies

- `python3` with `PyYAML`
- `config/units.yaml`

---

## Changelog

- 2026-07-13 — Initial implementation
