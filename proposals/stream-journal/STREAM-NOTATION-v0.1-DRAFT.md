# STREAM Notation — v0.1 draft

**Status:** design draft. It is a companion to [STREAM Journal v0.1](STREAM-JOURNAL-v0.1-DRAFT.md) and is tracked in [issue #1](https://github.com/babbworks/ww-standard/issues/1).
**Trial:** [`reference/notation.py`](reference/notation.py), [`reference/test_notation.py`](reference/test_notation.py) and [`reference/measure.py`](reference/measure.py), with the vocabulary in [`reference/vocab/notation-0.1.json`](reference/vocab/notation-0.1.json).

The Journal draft settles what a record claims. This note works out the smallest form a record can take, and the smallest act worth recording, given the aim the inquiry set: understanding how time, energy and compute are allotted across human, machine and agent work.

---

## N1. Requirements

| # | Requirement | Where it comes from |
|---|---|---|
| R1 | Few bytes stored and sent | Storage and bandwidth on constrained devices (ZAKO) |
| R2 | Little work to parse and aggregate | CPU time and energy per record |
| R3 | Few tokens when a model reads the log | Agent cost and context limits |
| R4 | Readable by a person, a shell pipeline and a model | The reason Stream works at all |
| R5 | Every form decodes to the same facts | Without this, compression changes meaning |
| R6 | Any level of a cascade can be recorded without losing what happened below it | A tool call hides hundreds of actions, and each of those hides thousands more |
| R7 | Time is the primary axis | Time is the resource every undertaking spends, and the common measure of efficiency |
| R8 | Lossless mapping to ZAKO and BitPads | Power-oriented OS, binary wire |

## N2. What the measurements say

### Where the bytes go

This breakdown is for the 35 canonical lines of the [example journal](examples/scenario/), 4,618 bytes in total:

| Part of the line | Share |
|---|---|
| JSON key names, quotes, braces and commas | 26.2% |
| Recorder and actor IDs, repeated on every line | 13.4% |
| Digests, legs, reasons and other values | 37.6% |
| Timestamp | 8.3% |
| Object | 6.6% |
| Action word | 4.9% |
| **Op letter** | **1.5%** |

The op letter is already as small as a field can be. Making the alphabet richer for it alone can't move the totals. The savings are in the key names, the repeated context and the absolute timestamps.

### Same records, different forms

Every form below decodes back to the canonical lines byte for byte. You can reproduce the table with `python3 reference/measure.py`.

| Data | Canonical | Canonical + gzip | Run form | Run form + gzip | PB1 binary |
|---|---|---|---|---|---|
| Mixed journal (35 records) | 4,618 B | 1,393 B | 2,537 B (55%) | 1,192 B | — |
| Agent task cascade (15 records) | 2,147 B | 457 B | 694 B (32%) | 309 B | — |
| Meter, 1,000 readings (seeded random) | 110,728 B | 7,514 B | 3,923 B (3.5%) | 1,342 B | 888 B |

Three conclusions follow:

1. **Stating context once and letting the record type set field positions does most of the work.** On periodic data, the run form is smaller than gzipped canonical lines, and it stays readable by eye, by `awk` and by a model without decompressing.
2. **gzip is the honest baseline for storage.** On mixed records it does about as well as the run form. What gzip can't do is stay readable, so the run form earns its place on R2–R4. On R1 alone, gzip is enough.
3. **Binary wins only for dense numeric streams.** PB1 is smaller than the run form plus gzip for periodic measurements. Everywhere else the difference is small.

### Measurements still to make

- **Parse cost.** In this prototype, decoding the run form is 2–3× slower than parsing canonical lines, because the decoder is pure Python and `json.loads` runs in C. A fair comparison needs native implementations of both. The structural argument for the run form is that it has fewer characters to scan and no key strings to hash, but that is not measured here.
- **Tokens.** Tokens are not bytes. A tokenizer merges frequent character sequences, so a common word such as `start` may cost one token, while a run of punctuation like `B(` may cost two. Long hex digests and large numbers split into many tokens however they are written. My hypothesis is that the token savings follow the byte savings in N2: dropped key names and dropped repeated IDs. I expect the glyphs themselves to save little. `python3 reference/measure.py --tokens` measures this with the Messages API `count_tokens` endpoint, including the one-time cost of the legend a model needs. The legend can be prompt-cached.

## N3. The atom: an act

The smallest unit worth recording is an **act**: an interval on the time axis, performed by an actor, of a kind, directed at an object, that consumed resources.

```
act = (start, end, actor, kind, object, use{quantity: amount}, evidence)
```

- **A point event is an act of zero length.** A measurement, a note or a posting happens at an instant.
- **An open/close pair is one act written in two halves.** The halves exist because work in progress must be visible, and because a missing close is evidence of a crash. The close record carries the act's `use`, so one record states the totals.
- **Time is the envelope, not a consumable.** Wall time does not add up under concurrency. Two tool calls running in parallel for 10 s took 10 s, not 20 s. Energy, CPU time, tokens, bytes, material and money do add up. They are *conserved*, and they balance like ledger quantities. The trial computes both:
  - **Utilisation** is CPU seconds divided by wall seconds. The agent task in the trial used 41 CPU-s in a 60 s envelope, or 0.683.
  - **Parallelism** is the summed child wall time divided by the parent's wall time.

This is the connection to BitLedger's discipline. A conserved quantity used by a parent equals the sum of its children's use plus a remainder. The remainder is stated, never assumed to be zero.

## N4. The cascade and recording resolution

"Run the tests" is one tool call. Underneath it are a process, thousands of system calls, millions of instructions and eventually register transfers. No log can keep all of that, and none needs to. What it needs is **totals that stay true at every level it chooses to record**.

### Levels

`R.open` takes a `span` from this list: `project`, `task`, `session`, `step`, `model`, `tool`, `process`, `syscall`. A stream declares the deepest level it records individually, and that is its recording resolution. Below the resolution:

- **Conserved quantities are rolled up into the parent's `use`.** Nothing is lost from the totals.
- **The number of acts not recorded individually is kept as a count.** The trial uses `412#`, meaning 412 sub-acts.
- **The parent's remainder is the work done below the resolution.** The attribution view reports it per quantity. A negative remainder, where children claim more than the parent used, is reported as over-attribution. It is never clamped.

From the trial (`test_notation.Attribution`):

```
R( task-42 k42 task
  R( k42.m1 model           ... R) ok 12000ti 900to
  R( k42.t1 tool
    R( k42.t1.p1 process    ... R) ok 51000000B ~470J 32.0xs
  R) k42.t1 ok 412# 52000000B ~480J 33.1xs        remainder: 1.1 xs, ~10 J, 1 MB, 412 unrecorded acts
  ...
R) k42 ok 610J 18000ti 2400to 41.0xs               remainder: 2.5 xs, ~55 J, 0 tokens
```

Two observations follow. First, every token is accounted for: model calls are already at their natural resolution. Second, 2.5 CPU-s and roughly 55 J happened between the recorded acts. That is the agent's own orchestration overhead, and it is now a number rather than an unknown.

### Measured and estimated

A value prefixed with `~` is an estimate. Anything mixed with an estimate becomes an estimate. Typical sources at each level:

| Level | Quantity | Typical source | Measured or estimated |
|---|---|---|---|
| host, socket | energy | Linux powercap / RAPL counters (`/sys/class/powercap/.../energy_uj`), or a PDU meter | measured for the host. A process share is `~`. |
| process, cgroup | CPU time | cgroup v2 `cpu.stat` (`usage_usec`) | measured |
| model call | tokens | API response `usage` (input, output, cache read, cache write) | measured |
| remote model | energy | provider disclosure only | `~` unless the provider gives evidence |
| syscall, instruction | counts and time | sampling profilers or eBPF | sampled, so `~` |

### Allotment

The inquiry's purpose is decisions about allotment: the same output from less input, or the surplus that would let output grow. Allotment fits the ledger without anything new being invented. A budget is an `L.post` that moves a quantity from a pool to an activity, such as `budget:host-01` to `run:k42`, 900 J. Consumption is the `use` on close. Variance is a view. The trial does not implement this. It is recorded here so the quantity codebook stays usable as ledger units (open question N10.4).

## N5. Three forms, one model

| Form | Self-contained per line | Readable | Use it for |
|---|---|---|---|
| **Canonical line**, the five-field v1 line with JSON `ctx` | yes | yes | The open segment, per-line hashing and addressing, v1 lenses |
| **Run form**, state-once text (N7) | no. A row needs its run header. | yes | Sealed segments, uploads, giving logs to a model, `awk` aggregation |
| **Binary**, PB1 and BitPads/C0-framed frames | no | no | Constrained links, dense numeric streams, ZAKO devices |

**Equivalence rule.** A form is valid only if decoding it reproduces the canonical lines byte for byte. Seals are computed over canonical bytes, so a segment stored in run form verifies after decoding. That allows hot segments to stay canonical while sealed segments are stored compact, with no loss of verifiability.

## N6. The alphabet

### Case and character class carry the role

Every printable ASCII character costs one byte. Case changes how many symbols a single byte can name (26 uppercase, 52 with lowercase, 94 printable), not how big the byte is. One printable character carries at most log₂ 94 ≈ 6.55 bits of distinction. ZAKO's 6-bit task code (64 verbs) therefore fits into one printable character, a fact that N6.5 returns to.

The run form gives each character class one job, so a reader can tell what a token is from its first character:

| Class | Role | Examples |
|---|---|---|
| Uppercase letter | Record family (OP) | `T` `B` `R` `M` `L` `A` `D` `S` |
| Punctuation | What the act does (the glyph) | `(` `)` `=` `~` `>` `/` |
| Lowercase letters | Attribute keys and quantity codes | `rec=` `xs` `ti` `kg` |
| Digits | Values | `38` `0.019` `+8` |

A type is the OP letter followed by its glyph, as a two-character token such as `B(`. With 26 families and 32 punctuation marks there are 832 possible types, well beyond what the vocabulary needs.

### Glyphs

| Glyph | Generic meaning | Types in `notation-0.1` |
|---|---|---|
| `(` `)` | open, close an interval | `B(` `B)` `R(` `R)`, plus `T(` for task start |
| `+` `-` `*` `.` | create, remove, modify, complete | `T+` `T-` `T*` `T.` |
| `=` `~` | observed value, estimated value | `M=` `M~` `D=` |
| `>` `<` `/` | post, reverse, amend | `L>` `L<` `L/` |
| `:` `!` `?` | annotate, alert or failure, query | `A:` and reserved |
| `[` `]` `x` | segment open, seal, rejection | `S[` `S]` `Sx` |

A glyph's meaning is generic, and the OP letter fixes the claim. `B)` closes a presence interval, while `T.` completes a work item. This keeps apart the distinction the inquiry found: an operator leaving a machine is not the same claim as a job being finished. It also follows ZAKO's own pictography design, where a Category Identity selects which 16-symbol codebook a 4-bit symbol is read from.

**Heritage, and where it was overruled.** Several of these characters had telegraph meanings first. In Morse, `+` is conventionally written for the prosign AR (end of message), `=` for BT (break) and `(` for KN (go ahead, named station only). The note doesn't use those meanings. Today's readers, and the models trained on today's text, read `+` as add and `=` as equals, and a notation is cheapest when it agrees with what its readers already know. The telegraph vocabulary survives where it is strongest, in the C0 controls (N6.3).

**Shell caution.** Most of these glyphs are shell metacharacters. They belong in files and on the wire. The CLI keeps accepting words (`ww stream emit B start …`), and the vocabulary maps word to glyph. Search with `grep -F`.

### C0 controls for the journal's own acts

The 32 C0 control codes come from teleprinter practice (Baudot–Murray, ITA2) and were standardised in ASCII. They are a ready-made vocabulary of communication acts, and the journal's bookkeeping acts map onto them one for one. In binary form, the journal uses the byte itself, with ZAKO's C0 Enhancement Grammar flags in the upper three bits: Priority, Ack-request, Continuation.

| Journal act | C0 | Note |
|---|---|---|
| Segment open | SOH 0x01 | Start of heading |
| Record accepted | ACK 0x06 | The receipt |
| Record rejected | NAK 0x15 | `S reject` |
| Seal | ETB 0x17 | End of transmission block |
| Checkpoint | SYN 0x16 | Synchronisation |
| Amend | SUB 0x1A | Substitute |
| Reverse or cancel | CAN 0x18 | |
| Alert | BEL 0x07 | |
| Query | ENQ 0x05 | |
| Pause, resume | DC3 0x13, DC1 0x11 | XOFF, XON |
| Stream, run, row, field | FS 0x1C, GS 0x1D, RS 0x1E, US 0x1F | The information separators, which were designed for exactly this hierarchy |

A run in binary is a GS-framed group of RS-separated rows with US-separated fields. Continuation=1 on a separator means more rows follow in this run. Content can't collide with framing, because these bytes never appear in printable text. This document proposes that framing but doesn't implement it. PB1 (Journal §10.3) is the binary form the trial does implement.

### Quantity tokens

A quantity is written as its value followed by its code in one token: `38J`, `-100kg`, `~470J`, `12000ti`, `0.019USD`.

- Codes come from a small codebook, `notation-0.1.json` → `quantities`. It uses SI symbols where they exist (`J`, `kg`, `B`) and short codes where they don't: `xs` for CPU seconds, `ti`, `to`, `tr` and `tw` for token classes, `#` for sub-act counts, and ISO 4217 codes for money.
- Each code states whether its quantity is conserved.
- Canonical values are stored in the code's base unit, as decimal strings. SI prefixes (`400mJ`) are accepted at input and used in display only, so encoding never has to do arithmetic, and so stays lossless.
- New physical processes add codes rather than keys. Irrigation would be `R) field-3 ok 42m3 18L 3.5h-labour`, once `m3`, `L` and a labour-time code are in the codebook.

### Mapping to ZAKO

| WE | ZAKO |
|---|---|
| OP letter | domain: a standard domain, or a Workwarrior distribution domain in 0xF0–0xFF |
| glyph | `task_code`, through the vocabulary's `zako` entries |
| quantity code | quantity type (6-bit), per domain |
| run header | BitPads session or batch header context |
| journal acts | C0 Enhancement bytes (N6.3) |

A direct arithmetic bijection is possible, for instance a SIXBIT-style table assigning each of the 64 ZAKO verbs a printable character. It isn't recommended. Only 32 punctuation marks exist, so most verbs would get arbitrary letters, and the notation would lose its readability. The vocabulary mapping is exact where ZAKO has the verb and explicit where it doesn't.

## N7. Run form

A run is one header line followed by rows. The header states, once, everything the rows share.

```
@<ts> [%ps=<n>] [%dt=<s> %op=<type> %obj=<object>] key=literal ...
```

| Header item | Meaning |
|---|---|
| `@<ts>` | Base time in absolute Unix seconds. |
| `%ps=<n>` | Producer sequence of the first row. Each later row is one higher, unless the row restates `ps` in its JSON, after which counting continues from there. Gaps and late arrivals cost a few bytes rather than a new header. |
| `%dt` `%op` `%obj` | A **template run**: fixed cadence, one type, one object. Each row is only its payload. `_` marks a missing slot, so time advances and no record or `ps` is produced. |
| `key=literal` | Shared context. Bare tokens are strings, unless they look like JSON numbers or `true`/`false`/`null`. Anything else is written as `key= <json>`. |

Ordinary rows have this shape:

```
<±seconds>[.<fraction>] <type> <object> <positional payload…> [{json for anything else}]
```

- **Time** is the whole-second delta from the previous row, followed by the record's own sub-second part when it has one (`tf`). Deltas may be negative, so journal order, which is address order, is never changed to suit time.
- **The positional payload** is set by the record type in `notation-0.1.json` → `positional`. For example, `M=` takes a quantity token, `R)` takes run, status, then use, and `L>` takes `account=quantity` legs. A value that can't be written as a token falls back to the JSON tail. The type decides, so the decoder never guesses.

Meter readings, the inquiry's example:

```
@1752300000 %dt=8 %op=M= %obj=pdu-4.out3 %ps=1 iv=8 rec=meter:pdu-4 scope=host:build-01
38J
41J
36J
```

That is 4 bytes per reading once the header is paid for. The canonical lines take 110 bytes per reading.

An inventory posting and its later correction:

```
@1752302100 %ps=1 actor=org:plant rec=human:acct-1
+0 L> run-r1 inv:raw=-100kg inv:product=92kg inv:scrap=8kg
+300 L/ run-r1 1:25 "weighbridge recheck" inv:raw=-100kg inv:product=91kg inv:scrap=9kg
```

## N8. Recommendations

1. **Keep the canonical line as the system of record.** Every other form must reproduce it byte for byte (N5).
2. **Adopt the run form** for sealed segments, uploads and anything handed to a model. Adopt PB1 for dense numeric streams. Use gzip on top for archival storage.
3. **Put a `use` vector on close records.** Treat wall time as the envelope, and the conserved quantities as a ledger with a stated remainder (N3, N4). This is now in `core-0.1.json` as `R.close` → `qtymaps: ["use"]`.
4. **Declare a recording resolution per stream.** Below it, record counts and totals rather than acts.
5. **Give each character class one role.** Scope glyphs by OP letter, and map them to ZAKO through the vocabulary, not arithmetic.
6. **Measure tokens before choosing glyphs over words for model-facing output.** `measure.py --tokens` does this. If words prove cheaper in tokens, the model-facing form keeps the run-form structure and swaps glyphs for words (`T.add` instead of `T+`). Measured on the trial data, words cost 7.4% more bytes on the mixed journal, 9.7% more on the cascade, and 0.2% more on template runs, which state the type once.

## N9. What the trial shows

- 11 notation tests pass alongside the 23 journal tests, for 34 in total, with 3 skipped without `WW_REPO`.
- **Lossless round trips:**
  - the mixed scenario journal;
  - a 1,000-reading template run;
  - cadence gaps;
  - negative and sub-second deltas;
  - values with quotes, braces and spaces;
  - strings that look numeric;
  - actions that have no glyph.
- **Attribution:** remainders per conserved quantity, estimate propagation, over-attribution, utilisation and parallelism.

**Not shown:** C0 binary framing, native parse cost, token counts, budgets and variance, and views that aggregate directly on the run form without expanding it.

## N10. Open questions

1. Should the deepest recorded level be declared per stream (in `S open`) or per run (in the header)?
2. Should the quantity codebook align one-to-one with ZAKO's 6-bit quantity types now, or after the codebook owners review it?
3. Should model-facing logs use glyphs or words? This turns on the token measurement in N2.
4. Should budget postings use the same account namespace as money (`budget:host-01` in `J`), or a separate resource ledger book?
5. **A discrepancy noted in passing.** The comment in `libzako-c0/zako_c0.h` says "29 unconditional + 4 conditional", which adds up to 33 of the 32 codes. The implementation treats SO, SI and ESC as conditional, plus DEL, which isn't a C0 code.
