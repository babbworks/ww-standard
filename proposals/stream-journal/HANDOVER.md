# STREAM: handover

Last updated 2026-11-23. Read this first in a new session, then the files it points to.

## 1. What this is

STREAM is a design, and now a first implementation, for **one append-only log of acts** by people, machines and AI agents, recording what each act consumed (time, energy, CPU, tokens, material, money). Balances, boards, traces, reports and the applications themselves are all **functions of that log**.

The aim is to understand resource allotment (time, energy and compute) for any undertaking, from a factory or farm to a company or a fleet of agents, and to keep records small. Each entry carries only what no wider context already says.

It grew out of an inquiry joining three things:
- **Workwarrior's Stream log:** `babbworks/workwarrior`, `services/stream/`.
- **ZAKO's universal verb codebook,** with the BitPads framing and BitLedger accounting: `babbworks/mobileos`, `ZAKO/`.
- **The research notes** in `babbworks/ww-standard`, `times-research/`.

## 2. Vocabulary (agreed; use these names)

| Term | Meaning |
| --- | --- |
| **Kernel** | One fixed page of rules for reading a log, named by hash on the log's first line (`STREAM/1 kernel=sha256:…`). |
| **Log** | The only authoritative store. Append-only, sealed in a hash chain, addressed by `segment:seq`. |
| **Entry** | One line: `<time> [P:]VERB[phase] <object> <canonical JSON>`. |
| **Verb** | One of 64 universal acts in 8 blocks of 8, from the ZAKO Codebook. |
| **Phase** | Instant, begin `(`, end `)`, sample `*`. Verb plus phase is one byte. |
| **Profile** | An uppercase letter: a local dialect of a verb (B = presence, T = tasks, D = derived signals, R = runs). |
| **Codebook** | Verbs, phases, units, payload layouts and replay behaviour. |
| **Register** | Things declared once: objects, actors, units, accounts, pools, prices, grants, views. It is folded from entries. |
| **Pack** | A hashed bundle of declarations, adopted with one BIND entry. Packs set up a log and install applications. |
| **View** | A pure function of the log. Applications and user interfaces are composed from views. |
| **Cache** | A saved view result. Disposable, and checked against seals. |
| **Books** | Balances of every conserved quantity, derived and never written. A Close commits to them by hash. |

The architecture is: **kernel → log → views → caches**, with packs adopted into the log. Only the log is authoritative.

## 3. Where everything is

**Repository:** `babbworks/ww-standard`, branch `claude/logging-standard-inquiry-ogvzkw`. Nothing is merged to `main`.

| Path | What it is |
| --- | --- |
| `proposals/stream-journal/STREAM-JOURNAL-v0.1-DRAFT.md` | First normative draft: claims, validation, journal, Books, ZAKO mapping, migration from Stream v1 |
| `proposals/stream-journal/STREAM-NOTATION-v0.1-DRAFT.md` | Notation, run form, cascade and attribution. N11–N14 record the agreed direction: ZAKO verb types, the lineage obligations, balance rules, resolved decisions. |
| `proposals/stream-journal/LINEAGE.md` + `lineage/lineage.json` + `lineage/lineage.py` | Lineage register: 37 terms (definition today → proposed), 6 worked record encodings, 65 decisions with pinned source lines and supersession links. `python3 lineage/lineage.py check` verifies every pin against git. `render` regenerates the Markdown. |
| `proposals/stream-journal/reference/` | Python trial: journal, notation, compact codec, `measure.py`. 34 tests. |
| `stream/` | **The Rust implementation** (section 5) |

**Shareable documents** (claude.ai; private to the owner until shared):
- *STREAM*, the full overview of about 8 pages, with lineage entries for every inventor: https://claude.ai/artifact/YQdRvxEe2N76UkLLEQ9L8y
- *The Log and the Books*, 3 pages: https://claude.ai/artifact/4ku35WwpgSZzRodiLZCXDj
- *The Relativity of Data*, postulates and tests: https://claude.ai/artifact/NmC7WjVkntXrLqa3LPUdrg
- *One Log*, a one-page summary for a technical investor: https://claude.ai/artifact/Rm2QUtegiwanS1jueXhWHc
- *STREAM interface mock-up*, a design canvas with a Now screen and the 64-act palette: https://claude.ai/artifact/MNJPCtqYdo2CZfMcghkFM1

**Proposal issue:** https://github.com/babbworks/ww-standard/issues/1. It covers only the first journal draft. Later decisions are not yet posted there.

## 4. Decisions agreed in conversation

All are in the notation draft (N11–N15) and `LINEAGE.md`. Items marked † were recorded on 2026-11-23 in N15 and the register as REL-1–2, KER-1–7, LOV-4–6, PAC-12–15, ZAK-13 (which supersedes ZAK-11) and INT-1–3. *The Relativity of Data* still lists five postulates; the sixth is not yet in that document.

- **Type = verb + phase,** with the domain declared per run. Workwarrior's letters become profiles. ZAKO mapping is by code, never by name.
- **The meaning stack:** state each fact at the widest scope where it holds. `ctx` is a staging area; keys that keep recurring are promoted to the Codebook.
- **The obligations of a record,** each named for an inventor:
  - Bundy: clock provenance and timezone per session.
  - Hollerith: objects declared once.
  - Baldwin: base version on mutations of shared objects, which replaces Frick's from-state.
  - Pacioli: movements, never balances; Close by hash.
  - Dey: how a value is known.
  - Babbage: class of resource, recorded by exception.
  - Lovelace: a view's result enters the log only as a signed estimate. Every verb declares its replay behaviour. Supersession applies to every claim.
- **Balance rules:** entries carry movements only. Close commits by a fixed-size hash. Snapshots are caches. A total is written only when it is independent evidence. Cumulative counters are delta-coded.
- † **The relativity of data,** six postulates:
  1. the record is invariant;
  2. encodings are equivalent;
  3. meaning is relative to the frame;
  4. order is relative;
  5. conserved quantities are invariant, value is not;
  6. the frame is recorded.
- † **Master formula:** `S = f_θ(L) = fold(step_θ, S₀, L)`. Indexed by time, `S(T) = Σ_{t≤T} e`.
- † **Everything is an entry** (Register, prices, grants, Codebook extensions, view definitions) except three things: the kernel, content stored by hash, and caches.
- † **Kernel:** a fixed preamble line; a hashed descriptor that travels with the log; genesis as the first entry. Eight bootstrap verbs: CREATE, BIND, GRANT, REVOKE, AMEND, OPEN, COMMIT, ACK. Kernel changes take effect by AMEND at a segment boundary.
- † **Packs:** adoption by one BIND; packs layer standard → domain → organisation; conflicting definitions are refused; software is composed from declarations (forms from the Codebook, menus from the Register, screens from views).
- † **Views:** pure (no clock, randomness or network); declared with inputs, method hash and runtime hash. A result is identified by (view, θ, address). Only published results enter the log, as estimates, and decisions cite them with `basis`.
- † **Books:** implied postings from `use` + `pool`; conversion is never pricing; capacity is conserved per actor; allotment is a posting into an allotment account.
- † **Text and wire:** the 95 printable characters are the log's alphabet; the 33 control characters are wire grammar only, keeping their ASCII meanings.
- † **Interface:** every element is a primitive made visible. Phase decides the control; the 8×8 grid is the shared palette; labels are local and codes universal.
- **Language:** Rust for the core and WebAssembly for views, chosen after comparing Go, Zig, C, Pascal, bash and assembly.

## 5. The Rust implementation (`stream/`)

`stream/` is a Cargo workspace that can move to its own repository unchanged.

```sh
cd stream && cargo test --workspace      # 18 tests, clippy clean
cargo build --release && target/release/stream kernel
```

| Crate | Status |
| --- | --- |
| `stream-kernel` | Done. Grammar, canonical JSON, decimals, 64 verbs, type byte, seals. `no_std`; builds for `wasm32-unknown-unknown` and `thumbv7em-none-eabihf`. |
| `stream-log` | Done. Segments, receipts and retries, rejections journaled as ACK, authority (roots and grants), Register, packs, seals, verification, gaps. |
| `stream-books` | Done. Balances (explicit legs and implied use), Close by hash, spans, attribution. |
| `stream-codec` | Partial. Run form with shared context and time deltas: about 3.2× smaller, lossless. |
| `stream-cli` | Done. `init`, `append`, `adopt`, `seal`, `close`, `verify`, `view`, `encode`, `decode`, `kernel`. |

Other pieces:
- `spec/kernel.descriptor` (the kernel itself);
- six conformance logs in `spec/conformance/` with an `EXPECTED` file;
- two packs, `si-units@1` and `agent-ops@1`.

Choices made in code:
- verb names made unique: `ASSIGN_CLASS` (0x05), `DELEGATE_EVENT` (0x37);
- an entry has four fields, so v1 logs need an importer;
- payload JSON allows integers only, and quantities are decimal strings with `~` marking an estimate;
- AMEND of a declaration carries `set`.

## 6. Open decisions (each has a recommended default)

1. **Phase as its own 2-bit field,** with paired verbs kept as instant claims. Recommended: yes.
2. **Letters as profiles.** Recommended: yes.
3. **How a type is spelled in readable form.** Recommended: verb names, with phase marks in the run form.
4. **Overlapping punches.** Recommended: flag them as over-attribution rather than splitting time.
5. **Several roots at genesis,** with a threshold. Recommended: allow it.
6. **A declarative view language.** Recommended: yes, with code as the escape hatch.
7. **Recording who viewed a view.** Recommended: off by default.
8. **Late entries for a closed period.** Recommended: an adjustment in the open period.

The overview document's "Open decisions" table has the full list.

## 7. Known problems in the sources

To be raised with their owners:
- **ZAKO verb names.** The Codebook uses ASSIGN twice (0x05, 0x20) and DELEGATE twice (0x0A, 0x37).
- **COMMIT's code.** Wire Conventions line 160 says COMMIT is 0x14; the Codebook says 0x15, where 0x14 is REMOVE.
- **C0 signal slots** are defined three different ways: the `libzako-c0` header, the BitPads-native document and Wire Conventions §8. The C0 header also counts 33 of 32 codes. Wire §8 puts LEDGER_ACK on ENQ and LEDGER_REJECT on ACK.
- **Workwarrior Stream v1.** Bundy and Board pair intervals by object, which merges overlapping shifts. Ingest dedup drops distinct actors, sorts the file in place, and reset truncates it.
- **Workwarrior's `punch-posting-v1`** would double-count actor hours under overlap (lineage entry PAC-11).
- **Lineage entries to verify.** The history behind the Frick and Cooper entries in the overview document couldn't be confirmed.

## 8. Suggested next steps, in order

1. ~~**Record the † decisions** in `lineage/lineage.json` (kernel, packs, views, caches, relativity, Books, wire, interface), then run `lineage.py check` and `render`.~~ Done: 85 decisions, 44 terms, 0 problems.
2. **Rust:** a WebAssembly view host (wasmtime, no imports), with `stream-books` moved onto it.
3. **Rust codec:** positional payloads and template rows, porting from `reference/notation.py`. The aim is the trial's ~28× reduction on meter data.
4. **Token measurement:** run `reference/measure.py --tokens` (needs the `anthropic` package and credentials), or an equivalent in Rust.
5. **Importers** for Workwarrior `stream.log` v1, Taskwarrior, Timewarrior and hledger.
6. **A second implementation:** run the Python trial against `stream/spec/conformance`.
7. **Make the interface mock-up interactive** and add a second domain, such as a farm.
8. **Post a progress comment on issue #1,** only if the owner asks.

## 9. Working conventions with this owner

- **Plain names.** Use Log, Books, Kernel, Packs, Views and Caches; don't split ideas into "Stream vs ZAKO".
- **Trace every change.** Record each change to a term or to how records are coded as a lineage mutation, with its source.
- **Shaping documents.** Ask a few sets of questions before writing a long document. Prefer shareable documents, with open questions and a recommendation for each.
- **Confirm before outward actions:** opening issues, commenting, pull requests. Commits and pushes go to the branch above only.
- **Ask about running code.** The owner once asked for no tests or runs during a design discussion. Check before running anything when the work is conceptual.
- **Repository gotcha:** `ww-standard/.gitignore` ignores `*.log`, so conformance logs must be added with `git add -f`.
