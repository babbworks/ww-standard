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

> **Partly superseded by N11.** The direction agreed after this draft builds the type from ZAKO verbs and a separate phase. The glyphs below survive as phase marks (`(` `)`) and as display aliases.

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

## N11. Type codes from ZAKO verbs (direction agreed)

**Status:** direction agreed on 2026-11-22. It is not yet in the trial code, and it supersedes the OP-letter-plus-glyph scheme in N6.

### Principle

Every piece of meaning is stated at the widest scope where it holds. Per-record context is whatever is left when nothing upstream already says it. ZAKO's codebooks are the widest scope available: agreed once, for every system, and free per record. The more of a record's meaning its type carries, the less context each record has to carry.

### The meaning stack

| Layer | Stated | Carries | Cost per record |
|---|---|---|---|
| Standard (ZAKO codebooks) | once, for every system | verbs, domains, quantity types, account pairs | 0 |
| Vocabulary version | per segment | what each type requires, payload order, validation | 0 |
| Session or stream | per segment | recorder, domain, recording depth, clock | 0 |
| Run | per header | actor, object, unit, cadence | 0 |
| **Record** | every line | when, which act, on what, how much | the irreducible part |
| Context (`ctx`) | when needed | anything not yet foreseen | the exception |

### The type

- **Verb.** This is ZAKO's 6-bit task code: what kind of act, universally.
- **Phase.** This is 2 bits, a dimension ZAKO does not have. It takes four values: instant, begin, end, and sample (a row in a regular series).
- **Domain.** This says what world the act is in. It is declared per run, because it rarely changes from record to record.

Phase plus verb is one byte. Phase removes the mismatches the inquiry found:

| Act | Type |
|---|---|
| Operator presence at a machine (Bundy) | ATTEND 0x36, begin and end. Neither half claims FINISH. |
| Model call | QUERY 0x38, begin and end |
| Tool call | ACT 0x3A, begin and end |
| Meter reading | MEASURE 0x2C, instant |
| Completing a work item | FINISH 0x23. This remains a separate lifecycle claim. |

### Printable form

The task code is block × 8 + index, so a block letter followed by an octal digit gives an exact printable form. `(` and `)` mark begin and end, and an instant has no mark. The letters are illustrative.

| Block | Letter | Examples |
|---|---|---|
| System | S | |
| Identity | I | |
| Structural | R | `R0` OPEN, `R5` COMMIT, `R6` AMEND |
| Exchange | X | `X2` ACK, `X4` PAY |
| Work | W | `W1` START, `W3` FINISH |
| Observe | O | `O1` NOTE, `O4` MEASURE, `O5` ESTIMATE |
| Schedule | C | `C6` ATTEND |
| Learning | L | `L0` QUERY, `L2` ACT |

### What the type implies

None of the following needs a context key:

- **The claim and its validation**, from the verb.
- **The payload's shape and order**, from the vocabulary.
- **The unit**, from the quantity type.
- **Measured or estimated**, from MEASURE versus ESTIMATE.
- **The direction of a transfer**, from the BitLedger account pair.
- **Privacy**, from the verb. ZAKO's JOURNAL already requires RESTRICT_FORWARD.
- **Which views use the record.**

The journal's own acts are verbs too: OPEN for a segment, COMMIT for a seal, AMEND for a correction and ACK for a receipt. The log describes itself in its own vocabulary.

```
@1752300000 dom=engineering rec=host:build-01 actor=agent:claude-07
+0    W1   k42                     work item started
+0.1  L0(  k42.m1                  model request begins
+4.3  L0)  k42.m1  12000ti 900to   response, tokens used
+0.5  L2(  k42.t1                  tool call begins
+35.5 L2)  k42.t1  33.1xs ~480J 412#
+20   W3   k42     41.0xs 610J     finished, with totals
```

### Payoff for allotment

Every act from every source carries the same verb axis. Energy, CPU time, tokens and wall time can therefore be summed by verb across people, machines and agents. Domain extensions (0xF0–0xFF) refine a claim locally without breaking that comparison.

### Context as a staging area

`ctx` holds meaning until it is understood well enough to codify. Track context bytes per record. When a key keeps recurring, promote it into a codebook entry, a type's payload, or a run declaration.

### Decisions still open

1. Adopt phase as its own 2-bit dimension, and keep ZAKO's paired verbs (START/FINISH, OPEN/CLOSE, SUSPEND/RESUME) as instant lifecycle claims?
2. Turn the Workwarrior op families into profiles that map onto verbs, with B as ATTEND, D as OBSERVE or ESTIMATE, and A as NOTE? The lineage names would survive as lens names.
3. Use block letter plus octal digit (exact), or mnemonic glyphs (readable, but not a direct bijection)?

## N12. What the lineage concepts require of a record: Bundy, Dey, Frick, Hollerith, Pacioli

**Status:** analysis agreed on 2026-11-23. Not yet in the trial code.

**The test.** A concept is only a lens if the lens can always produce a correct answer from records made without it. If the information it needs exists only at the moment of the act, and is lost unless captured then, the concept has to change how records are created. All five pass this test in one specific place each, so each contributes one obligation.

### Hollerith: declare the card before punching it

- **What his system depended on.** The card layout and the category codes were fixed before any card was punched. That is why tabulating worked.
- **What breaks today.** Objects are free strings, and their attributes (`name`, `proj`, `tags`) are repeated in `ctx` on every line, though they describe the object, not the act. The Hollerith grid gets unreliable columns: `lathe-01` and `Lathe01` become two columns, and a renamed project splits its history.
- **Change.** An object is introduced once by a declaration record (CREATE or BIND) with its kind and attributes. Acts then refer to it by a short reference. The positional payload for each type is the other half of the card design.
- **Cost per record:** negative.
- **Not adopted:** "sorted by column 1" as a storage rule. It conflicts with append order being the address. Time order stays a view.

### Bundy: the clock, not the worker, supplies the time

- **Interval identity.** An interval gets its identity when it begins, so overlapping intervals pair correctly. The begin/end phase in N11 comes from Bundy.
- **Trusted time.** The worker supplies the act, and the clock supplies the timestamp. Records today don't say where their time came from. An offline phone, a server receipt and a person typing "I left at 5" all arrive as the same integer.
- **Change.** Each session declares its clock source, precision and sync state, plus its timezone. Cooper and Bundy need local time of day, and Cooper currently angles by UTC. A time supplied by a person is marked as a claim.
- **Cost per record:** zero, because it is declared per session.

### Frick: a transition names the state it leaves

- **What the lens shows today.** It chains actions per object.
- **What breaks.** A record that says only where an object went can't reveal a missed transition, or two agents both doing `start` on the same task. Producer sequence gaps catch losses per recorder, but nothing catches them per object.
- **Change.** The vocabulary defines the state machine for each object kind. A transition on a shared object carries the state it leaves. A mismatch is accepted and reported, consistent with Journal §5.3.
- **Cost per record:** about 1 byte, for objects with several writers only.

### Pacioli: every quantity moves from somewhere

- **What the lens does today.** It counts events labelled credit or debit, so a start followed by a stop gives a balance of 2.
- **Pacioli's real contribution.** No quantity is a bare number. It is a signed movement between accounts, and it balances.
- **Already adopted:** postings that balance per unit, `use` vectors, and stated remainders.
- **Refinement 1.** A `use` vector moves resources from a pool declared in the run header, so consumption is double-entry at no cost per record.
- **Refinement 2.** A period close (COMMIT, or Tally) is a record asserting balances at a point in time, which lets later edits to history be detected.
- **Retire:** the count-based lens. Replace it with a balance view.

### Dey: say how a value is known

- **What breaks today.** D samples are derived indices computed in the browser from 11 weighted signals with exponential smoothing, and the method isn't recorded. When there are no samples, the lens builds lookalike values from event density without labelling them.
- **Change.**
  - **The sample phase.** A continuous signal is a declared series with a cadence, and missing samples are marked as gaps.
  - **Derived values are estimates**, recorded with their method, version and window.
  - **Views label approximations** in their output.
  - **Behavioural signals about a person are restricted**, with the restriction implied by verb or domain, as ZAKO's JOURNAL does.
- **Cost per record:** zero.

### The obligations of a record

| A record must say… | From | Usually stated |
|---|---|---|
| when, by whose clock | Bundy | session |
| which act, and its card layout | Hollerith and ZAKO verbs | standard and vocabulary |
| on what: a declared object | Hollerith | once per object |
| from what state | Frick | per record, shared objects only |
| how much, from where, balanced | Pacioli | values per record; pool per run |
| how it is known: measured, estimated or claimed | Dey and Bundy | type and run |

**Burroughs, Felt, Cooper, Grant and Baldwin** need no change to record creation, beyond what these obligations already cover. N13 examines them in full.

### Decisions open

> **Resolved in N14.** Each decision now has a lineage ID in [LINEAGE.md](LINEAGE.md). Decision 2 (the "from" state) was superseded by a base version, BAL-1.

1. Should objects be declared, with acts referring to them by reference? This is the largest structural change.
2. Should transitions on shared objects carry a "from" state? This is the only per-record cost.
3. Should session-level clock provenance and timezone be required?
4. Should the count-based Pacioli lens be retired in favour of a balance view?

## N13. The remaining lineage concepts: Burroughs, Baldwin, Grant, Felt, Cooper, Babbage, Lovelace

**Status:** analysis agreed on 2026-11-23. The test is the same as in N12. The IDs refer to [LINEAGE.md](LINEAGE.md).

| Concept | What it is | Effect on record creation | IDs |
|---|---|---|---|
| **Burroughs**: accumulation | The adding machine printed every entry as it added, so the total came with its proof. Its governor made the result the same however hard the handle was pulled. | None. The printed tape is provenance, and the governor is retry identity. Both are already journal obligations. | BUR-1, BUR-2, BUR-3 |
| **Baldwin**: deltas, reversibility | A modification carries new values. Checking or undoing it needs to know what it was applied to. | A mutation of a shared object carries the **version it was based on**. This replaces Frick's "from" state, since the state can be worked out from the version, and it exposes concurrent writes. Snapshots bound replay cost, but they are caches, not records (N14). | BAL-1, BAL-2 |
| **Grant**: derived metrics | Aggregates and second differences | None. A *published* metric enters the journal only as an attributed act (LOV-1). | GRA-1 |
| **Felt**: density | Event-count density depends on how deep you record. | None. Redefine Felt as rates of conserved quantities (power, utilisation, tokens per second). Event-count density is secondary and labelled with its recording depth. | FEL-1 |
| **Cooper**: radial field | Time of day as angle, intensity as radius. The repository names no historical precedent. | None beyond the session timezone (BUN-4). The angle must come from local time. | COO-1 |
| **Babbage**: Engine | Store, Mill, Barrel and cards are lens and scheduling architecture. | None. | — |
| **Babbage**: Difference Engine | A smooth series is cheapest as a starting value plus differences. | Encoding only, in the run form and binary. A cumulative counter keeps its reading in canonical form and is delta-coded in compact form. | BAB-2, BAB-3 |
| **Babbage**: *Economy of Machinery and Manufactures* (1832) | Divide work by the skill each part needs, and pay for each part only at that level. This is the allotment question itself. | **New obligation: by what class of resource.** It is declared per actor, and recorded per act only when what served the act differed from what was declared. | BAB-1 |
| **Lovelace**: "no power of originating" | Lenses never write facts. | A lens result enters the journal only as an attributed ESTIMATE or COMMIT, with method, version and basis. | LOV-1 |
| **Lovelace**: symbols with defined relations | Universal verbs need universal replay rules. | Every verb declares its replay behaviour from a closed set. | LOV-2 |
| **Lovelace**: successive approximation | Estimates are refined. | Supersession applies to every claim, not only postings. | LOV-3 |

### The obligations of a record, updated

| A record must say… | From | Usually stated |
|---|---|---|
| when, by whose clock | Bundy | session |
| which act, its layout, and its replay behaviour | Hollerith, ZAKO, Lovelace | standard and vocabulary |
| on what: a declared object | Hollerith | once per object |
| based on which version | Frick, Baldwin | per record, shared objects only |
| how much, from where, balanced | Pacioli | values per record; pool per run |
| how it is known: measured, estimated or claimed | Dey, Bundy | type and run |
| by what class of resource | Babbage | actor declaration; per act on deviation |
| if derived: who derived it, how, and from what | Lovelace | the deriving act |

## N14. Balances without restating them, and resolved decisions

**Status:** agreed on 2026-11-23.

### Balance rules

Writing a balance on every record, as a bank statement prints "balance after" on each line, is ruled out.

1. **Records carry movements, never balances (PAC-8).** Running balances, remainders and rates are computed by views.
2. **A period close commits to balances without restating them (PAC-7).** The close record carries a hash of the full balance table, the period and a record count. Its size is fixed, however many accounts exist. A reader replays the period and checks the hash. A hash tree is optional, for proving one account's balance without revealing the rest.
3. **Snapshots are caches, not records (BAL-2).** Saved object states and balance tables live outside the journal. Each is keyed to a journal address and checked against the close hash, and each can be dropped and rebuilt.
4. **A total is written only when it is independent evidence (PAC-9).** A parent's `use` is recorded only if something measured it separately. Otherwise the view adds up the children.
5. **Exception: cumulative counters (BAB-3).** The canonical record keeps the counter's reading, so a lost sample loses nothing. The compact form stores only the change from the previous reading.

The journal therefore holds one balance-related record per period, and its size doesn't grow with the number of accounts.

### Resolved decisions

| Decision | Resolution | ID |
|---|---|---|
| N12.1 Declare objects | **Yes.** Attributes move into one declaration (CREATE or BIND). Acts carry the full stable ID in canonical form, and short references only in compact forms. Renaming is one modification. Undeclared objects are accepted and flagged. | HOL-1, HOL-2, HOL-3 |
| N12.2 "From" state on transitions | **Superseded** by a base version. | FRI-1 → BAL-1 |
| N12.3 Clock provenance and timezone | **Yes, required per session.** The session declares the clock source (synced device, free-running device, GPS, server receipt or person), its precision, the last known offset at sync, and an IANA timezone. "Unknown" is allowed but flagged. Receipt time serves as a second clock for estimating skew. | BUN-4, BUN-5 |
| N12.4 Retire the count-based Pacioli lens | **Yes.** It becomes a balance view under the Pacioli name. Activity per object moves to Burroughs and Hollerith counts. | PAC-5 |
| N13.1 Base version | **Yes.** It applies to objects declared shared, and the implied new version is base + 1. Two accepted changes on the same base are reported as a conflict, not rejected. It costs 1–2 bytes in compact form. | BAL-1 |
| N13.2 Class of resource | **Yes, by exception.** The declared class is part of the actor. A per-act class appears only when the class that served the act differs from the declared one. | BAB-1 |
| N13.3 Replay behaviour per verb | **Yes, required.** It comes from a closed set: instant observation, interval boundary, state transition, quantity movement, supersession, period close, declaration, annotation. That is 3 bits in the vocabulary. | LOV-2 |
| N13.4 Supersession for every claim | **Yes, within the same behaviour.** Only the latest version can be corrected. A superseded measurement keeps its original reading. Supplying a missing record is still reconciliation, not supersession. | LOV-3 |
| N13.5 Felt as rates | **Yes, as its primary meaning.** Event-count density stays as a secondary measure, labelled with its recording depth. Neither is stored. | FEL-1 |

N11's three decisions remain open: ZAK-1, ZAK-2 and ZAK-3 in the register.
