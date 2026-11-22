# STREAM Journal — v0.1 draft

**Status:** proposal draft, not ratified. Trial implementation in [`reference/`](reference/), results in [`README.md`](README.md#trial-results).
**Extends:** [Stream Event Format v1](https://github.com/babbworks/workwarrior/blob/main/dev/schemas/stream-event-v1.md). Every journal line is a valid v1 line. The five positional fields stay frozen.
**Borrows from:** ZAKO Codebook v1 (vocabulary), BitPads (framing), BitLedger (balance discipline), ZAKO Wire Conventions (corrections, sealed history).

The keywords MUST, SHOULD and MAY are used in their RFC 2119 sense.

---

## 1. What this standard is for

Stream v1 records activity. The journal makes the stream a **system of record**: task state, punch intervals, agent run traces, measurements and account balances are all rebuilt from the same accepted history, and nothing outside the journal is needed to reproduce them.

Five principles from the inquiry, each of which a later section makes testable:

1. **The record type decides the claim.** `OP.action` names a versioned definition. The definition says what the record asserts, what context it needs, and what the journal must check before accepting it (§4, §5).
2. **Who did it, who wrote it down, and who signed it are different parties** (§5.1).
3. **Accepted history is append-only and addressable by position.** Occurrence order is a view and is never written back (§6).
4. **Validate what a record claims on its own. Report what history contradicts.** An offline producer's late records must not be refused because state moved on. Postings and corrections are the exception, because a wrong balance cannot be repaired by a view (§5.3).
5. **A compact form is valid only if it decodes to the same canonical lines** (§10).

## 2. Terms

| Term | Meaning |
|---|---|
| occurrence | Something that happened. It has an occurrence time (field 1). |
| record | One journal line describing one occurrence. |
| claim | What a record asserts, as fixed by its definition. |
| definition | The entry for `OP.action` in a vocabulary version. |
| recorder (`rec`) | The authenticated party that submitted the record. Set by the journal. |
| actor (`actor`) | The party that performed the occurrence. |
| signer | The holder of a key over source bytes retained as evidence (§11.3). |
| segment | One append-only file of records, opened by `S open` and closed by `S seal`. |
| address | `<segment>:<seq>`, the 1-based line number in the segment. Never stored on the line it names. |
| receipt | The journal's answer to a submission: `accepted`, `duplicate` or `rejected`, with an address. |
| view | A deterministic function of committed records and versioned rules. |

## 3. Records

### 3.1 Line format

Unchanged from v1:

```
<unix_ts> <OP> <action> <object> <ctx_json>
```

### 3.2 New reserved `ctx` keys

v1 readers already tolerate unknown keys, so all of these are additive.

| Key | Type | Set by | Meaning |
|---|---|---|---|
| `rec` | actor id | journal | Recorder. A producer MAY send it. The journal rejects the record if the value differs from the authenticated submitter. |
| `ps` | integer ≥ 1 | producer | Producer sequence, monotonic per recorder. Retry identity and gap detection (§6.5, §6.7). |
| `rk` | string | producer | Retry key, for producers that cannot keep a counter. Gives no gap detection. |
| `run` | string | producer | Identifies the interval or run that boundary records pair on (§7.3). |
| `parent` | string | producer | `run` of the enclosing run. |
| `ref` | address | producer | The record that this one corrects. |
| `tf` | integer 0–999999 | producer | Microseconds within the second in field 1. |
| `q`, `u` | decimal string, unit | producer | Quantity and unit. Quantities are always decimal strings, never JSON numbers. |
| `legs` | array | producer | Posting legs `{a, q, u, book?}` (§8). |
| `basis` | address[] | producer | Records an estimate or charge was derived from. |
| `method` | string | producer | Versioned method for an estimate, such as `cpu-share@1`. |
| `tariff` | string | producer | Versioned tariff a posted charge was priced under (§8.4). |
| `use` | object | producer | Resources an act consumed, on close records: quantity code → decimal string, with `~` marking an estimate. See [STREAM Notation](STREAM-NOTATION-v0.1-DRAFT.md) N3–N4. |
| `scope` | string | producer | What a measurement covers, such as `host:build-01`. |
| `iv` | integer | producer | Measurement interval in seconds. |
| `status`, `span`, `reason` | string | producer | Defined per operation (§4.3). |

### 3.3 Canonical form

The journal writes `ctx` with sorted keys and no insignificant whitespace. The canonical line is what retry digests, seals and compact-encoding equivalence are computed over.

## 4. Vocabulary

### 4.1 Lookup

The lookup key is `OP.action`. No second type field is added. A vocabulary is a versioned document, such as [`core@0.1`](reference/vocab/core-0.1.json). Each segment declares the version that governs it in its `S open` record, so a line carries no version bytes.

### 4.2 Claim kinds

A definition selects one kind. The kind selects the validator and the view rules.

| Kind | Claim | Journal checks | Examples |
|---|---|---|---|
| `transition` | A subject changed state | Required keys | `T.add`, `T.done` |
| `boundary` | An interval opened or closed | Required keys, pairing key resolvable | `B.start`, `R.close` |
| `observation` | A value was measured or estimated | Decimal quantity, unit, `basis` resolves | `M.measure`, `M.estimate` |
| `posting` | A balanced transfer happened | Balance per (book, unit), §8.2 | `L.post` |
| `correction` | A prior posting is replaced or reversed | `ref` is the head of its chain | `L.amend`, `L.reverse` |
| `annotation` | Free text about a subject | Required keys | `A.note` |
| `system` | Journal bookkeeping | Never accepted from producers | `S open`, `S seal`, `S reject` |

### 4.3 Op codes

v1 codes keep their meaning. The draft adds three:

| OP | Name | Why it is separate |
|---|---|---|
| `M` | Measure | General quantified observations with a unit. `D` stays Dey's derived behavioural signal. This settles the open question about reusing `D` for measurement. |
| `L` | Ledger | Balanced postings and corrections. v1's ledger adapter records that a posting happened and omits the amount, so its records can't support balances. |
| `R` | Run | Spans of machine or agent work, such as sessions, model calls, tool calls and processes. A `B` punch claims that an actor is present on an object. An `R` span claims that a unit of work is in flight. |

The earlier proposals `X transform` and `S context` are not adopted. A transformation is a posting with legs in several accounts (§8.1). Shared context belongs to the compact encoding (§10), because the canonical journal keeps every line self-contained.

### 4.4 Pairing

A `boundary` definition names a pairing key. `B` pairs on `run` when present, otherwise on `(object, actor)`. `R` always pairs on `run`. v1 paired on `object` alone. With that rule, two operators on one machine merge into a single interval (§12.1).

## 5. Validation

### 5.1 Recorder, actor, signer

- The journal sets `rec` to the authenticated submitter.
- `actor` defaults to `rec`. A recorder MAY record for a different actor only if a grant allows it. For example, `host:build-01` may record for `agent:*`. An operator may record only for themselves.
- Readers resolve the actor as `ctx.actor`, then `ctx.rec`, then the v1 fallback `profile:<prof>`.
- Signatures are evidence about source bytes. They don't make anyone an actor (§11.3).

### 5.2 Intrinsic checks

The journal checks every record against its definition, using nothing but the record and the addresses it references:

1. A definition exists for `OP.action` in the segment's vocabulary.
2. Required keys are present, and enumerations hold.
3. Every quantity is a finite decimal string.
4. Every `ref` and `basis` address is already committed.
5. The recorder holds authority for the actor.
6. Postings balance (§8.2).

### 5.3 Historical consistency is reported, not enforced

The journal accepts two opens on the same pairing key, a close without an open, or a task finishing before it started, as long as each record is intrinsically valid. Views flag these as anomalies, and an attributable correcting record resolves them. v1 already requires that "nothing may auto-close a punch silently". The journal extends that rule: nothing may refuse a late truthful record because the board has moved on.

Corrections are the exception. A correction MUST name the current head of its chain. If two amendments of one posting were both accepted, both would count, so the second is refused.

## 6. Journal

### 6.1 Append only

The journal MUST NOT sort, rewrite, truncate or delete committed lines. A view MAY present any order, but it never writes that order back. This replaces v1's in-place ingest sort.

### 6.2 Segments

The first line of a segment is:

```
<ts> S open seg-000002 {"prev":"<chain hash of the previous seal>","stream":"main","vocab":"core@0.1"}
```

Here `ts` is the arrival time. The open record therefore states when the segment began, and the occurrence times of the records that follow can be earlier.

### 6.3 Seals

A seal closes a segment:

```
<ts> S seal seg-000001 {"chain":"<H(prev ‖ sha256)>","n":32,"sha256":"<sha256 of every byte before this line>"}
```

The chain begins at 64 zeros. Anyone holding the segments can recompute every seal. Anchoring a chain hash outside the journal, through a signed checkpoint or a ZAKO Tally, is what makes later tampering detectable to a third party. That anchor is out of scope for v0.1.

**Reset becomes an epoch.** v1 `reset` truncates the log. Under the journal, a reset seals the current segment and opens the next one with `"epoch": true`. Views start from the latest epoch by default, and the earlier history is still there.

### 6.4 Batches are transport

A batch carries many records. The journal validates and commits each one independently. A claim that must be atomic, such as a multi-leg transaction, is a single record, so the "pending compound group" state in ZAKO Wire Conventions is never needed.

### 6.5 Receipts and retries

The retry identity is `(rec, ps)`, or `(rec, rk)` if the record has no `ps`.

| Incoming | Retry identity already holds | Result |
|---|---|---|
| Same claim | Accepted record | `duplicate` with the original address |
| Same claim | Rejection | `rejected` with the original address and reason |
| Different claim | Anything | `rejected` as a conflict. Nothing is written. |
| Not seen before | | Validated. The result is `accepted`, or `rejected` with an `S reject` record written. |

"Same claim" means the canonical line, with `rec` set, is byte-identical. Two identical occurrences with different `ps` values are two records.

An `S reject` record stores the recorder, the retry identity, the claim digest and the reason. It doesn't store the rejected payload. Journaling rejections keeps the producer's sequence fully accounted for (§6.7). A producer that fixes a rejected claim resubmits it under a new `ps`.

A record with no `ps` or `rk` cannot be retried safely, so it MAY be refused at profile level. Legacy import assigns an `rk` (§12.2).

### 6.6 Derived state

Everything beyond the segment files can be rebuilt from them. That includes the retry index, chain heads, the superseded-posting map and every view. Conformance requires rebuilding from the segments alone (§13).

### 6.7 Capture completeness

For each recorder, the gaps in `ps` (counting both committed records and `S reject` records) are the records the journal never received. Gaps are a view, and so they can close when spooled records arrive. A balance check can't detect an omitted pair of equal and opposite entries, but sequence gaps can. That makes them the second leg of completeness, alongside receipts.

## 7. Replay and views

### 7.1 Determinism

A view is a function of:

- the committed records up to an address;
- the vocabulary version of each segment;
- a versioned rule set, such as `tariff@t2` or `cpu-share@1`.

`S` records are bookkeeping. Views exclude them at one point, which matches v1's `replay_load`, except that the capture-gap view reads `S reject`.

### 7.2 Provenance

Every figure a core view produces carries the addresses it came from. A balance lists its contributing postings. An amendment walks back through its `ref` chain to the original evidence.

### 7.3 Core views

| View | Rule |
|---|---|
| Intervals (`B`, `R`) | Pair by pairing key in occurrence order. Ties break on `tf`, then address. Report open intervals, reopens and orphan closes. |
| Tasks | Fold the `T` transitions for each object. |
| Balances | Sum legs of postings and amendments that have not been superseded, per (book, account, unit). |
| Capture gaps | §6.7. |
| Timeline | Occurrence order. Ties break on `tf`, then address. |

The v1 lenses keep working on journal lines. They can adopt the pairing rule in §4.4 without changing the format.

## 8. Accounting

### 8.1 Observation, posting, reconciliation

These are three different claims, and the journal keeps them separate:

```
M measure bin-7      {"q":"92","u":"kg",...}                          observation: a scale said 92 kg
L post    run-r1     {"legs":[{"a":"inv:raw","q":"-100","u":"kg"},
                              {"a":"inv:product","q":"92","u":"kg"},
                              {"a":"inv:scrap","q":"8","u":"kg"}],...}    posting: stock moved
L amend   run-r1     {"ref":"1:25","reason":"weighbridge recheck",
                      "legs":[...91 kg product, 9 kg scrap...]}           reconciliation: corrected, original kept
```

### 8.2 Balance rule

- Each posting MUST have at least two legs. Every leg has a non-zero decimal quantity.
- Within one posting, the legs MUST sum to zero for each (`book`, `u`) pair. `book` is the accounting boundary, and leaving it out means the default book.
- No conversion happens implicitly. Moving value between units takes explicit conversion legs in each unit, in the same way as hledger's equity conversion postings.
- Balance is checked per posting, never per batch. Two postings that are wrong by +5 and −5 are both refused, even when they arrive together.

### 8.3 Corrections

- `L.amend` replaces the posting at `ref` with its own legs.
- `L.reverse` cancels the posting at `ref` and carries no legs. It is final.
- Both MUST target the head of the correction chain, and both MUST carry a `reason`.
- The original posting stays in the journal unchanged. Balances skip it because it has been superseded.

### 8.4 Charges and tariffs

A cost is derived from usage (`M.measure` records) and a versioned tariff. A view can price the same usage under any tariff without writing anything. A provisional charge becomes part of the record only when it is posted as an `L.post` with `basis` and `tariff` keys. A later invoice confirms or amends that posting.

## 9. Agent run profile

| Concern | Record |
|---|---|
| Session, model call, tool call, process | `R.open` / `R.close`, with `run`, `parent`, `span`, `status` and `tf` |
| Tokens | `M.measure` on the run, with `u` set to `tok.in`, `tok.out`, `tok.cache_read` or `tok.cache_write`, plus `provider` and the resolved `model` |
| Host energy | `M.measure` from the meter, with `scope` and `iv`. The meter is the recorder. |
| Energy attributed to a run | `M.estimate` with `method` and `basis`. It is never recorded as a measurement. |
| Remote model energy | Requires evidence from the provider. Without it, an estimate with a declared method. |
| Cost | Priced by a view (§8.4). Posted when it is billed or accrued. |

The host records for the agent (`rec=host:…`, `actor=agent:…`) under a grant.

## 10. Compact encodings

### 10.1 Equivalence

A compact encoding is valid when decoding it reproduces the canonical lines byte for byte. Size comparisons count the whole batch: the context dictionary, the frame headers, the payload and the exceptions. An encoder MUST refuse input it cannot reproduce exactly, such as `"38.0"` at exponent 0, records off cadence or a broken `ps` run. It falls back to a less compact form instead.

### 10.2 Context dictionary

A batch carries a list of shared-context entries. Each entry is the canonical JSON of `op`, `action`, `obj` and every `ctx` key other than `ps` and `q`. Blocks refer to an entry by its index. Shared numbers for stable concepts belong in the vocabulary, and local numbers (machines, operators, models) belong in the dictionary.

### 10.3 PB1 — periodic measurement block

PB1 carries `M.measure` records that share one context entry and a fixed cadence. Its layout is in [`reference/compact.py`](reference/compact.py):

- a 23-byte header with `t0`, cadence, `n`, bits, decimal exponent, base and `ps0`;
- an exception list, where each entry marks a missing slot (3 bytes) or an out-of-range value (7 bytes);
- the values, bit-packed.

A missing slot consumes no `ps`.

Trial figures for the three-reading example (38, 41 and 36 J, 8 s apart) and for 1,000 readings are in the [README](README.md#trial-results).

### 10.4 Relation to BitPads

PB1 is a proposal for a BitPads profile, not a BitPads frame. A BitPads Wave per sample suits small signals that need low latency. A packed block suits periodic data that needs low bandwidth. In both cases the profile MUST state what the session or batch header carries, so that §10.1 can be checked.

## 11. ZAKO and BitPads mapping

### 11.1 Verbs

The vocabulary records the closest ZAKO `task_code` for each definition and how faithful the mapping is:

| Definition | ZAKO | Fidelity |
|---|---|---|
| `T.start`, `T.done` | 0x21 START, 0x23 FINISH | exact |
| `M.measure`, `M.estimate` | 0x2C MEASURE, 0x2D ESTIMATE | exact |
| `A.note` | 0x29 NOTE | exact |
| `L.amend` | 0x16 AMEND | exact |
| `T.add`, `T.modify`, `T.delete` | 0x20 ASSIGN, 0x22 PROGRESS, 0x32 CANCEL | partial |
| `R.open`, `R.close` | 0x38 QUERY (model) or 0x3A ACT (tool), 0x39 RESPOND | partial. A failed call is not a response, and a session has no verb. |
| `D.*` | 0x28 OBSERVE | partial. Dey values are derived indices. |
| `L.post` | BitLedger component and account pair | structural. A verb such as PAY or INVOICE is an optional business claim on top. |
| `B.start`, `B.stop` | none | A presence interval is not START or FINISH. |

### 11.2 Extension path

The ZAKO task_code space is 6 bits and fully allocated in Codebook v1. For verbs that aren't standard, §8.1 of the Codebook points distributions to a custom domain with extension byte 0xF0–0xFF. **Proposal:** Workwarrior declares one distribution domain, a "Stream" domain at an 0xF0-range value to be agreed, and carries the presence-interval and run-span semantics there. Implementations that don't recognise the domain store those records without processing them.

Mapping MUST be by number, never by name. The Codebook uses the name ASSIGN for both 0x05 and 0x20, and DELEGATE for both 0x0A and 0x37.

### 11.3 Signed source frames

When a source frame is signed, the journal keeps the original bytes and the signed context as evidence. They go in a content-addressed store referenced from `ctx.sig = {"alg", "key", "bytes": "<sha256>"}`. The normalised record is still the canonical claim, and the evidence lets anyone check that the normalisation was faithful.

### 11.4 Known divergences to settle before claiming compatibility

These were carried over from the inquiry and are unresolved:

- Some wire-document examples give verb numbers that differ from the Codebook.
- The direction bit means different things in the wire document and in the BitLedger code.
- The built session codec uses a 4-bit task category, while the 6-bit packing exists only in source that isn't built.
- The ledger daemon calls the store API with mismatched signatures.

## 12. Migration from Stream v1

### 12.1 Behaviour that changes

| v1 behaviour | Journal behaviour | Trial evidence |
|---|---|---|
| Dedup keys on `(ts, OP, action, object)`, so a distinct actor at the same second is dropped | Retry identity is `(rec, ps \| rk)` | `LegacyComparison.test_v1_dedup_drops_distinct_actor` |
| Bundy and Board pair on `object`, so overlapping operators merge | Pair on `run`, otherwise `(object, actor)` | `test_v1_bundy_merges_overlap`: v1 reports one 10-minute interval where 20 + 40 minutes happened |
| Ingest sorts the file in place | Append only. Order is a view. | — |
| `reset` truncates | Seal the segment and start an epoch | — |
| Ledger events carry no amount | `L.post` carries legs | — |

### 12.2 Importing a v1 log

A v1 log is imported once, under the recorder `import:ww-v1`, which holds a grant for every actor. Each line keeps its fields and gains `rk = <first 16 hex of sha256(line)>.<n>`, where `n` counts earlier occurrences of the identical line. Repeated identical lines therefore stay distinct records, and re-running the import changes nothing.

## 13. Conformance

An implementation conforms when, given the scenario in [`reference/test_conformance.py`](reference/test_conformance.py), it does all of the following:

1. Returns the original receipts when a spool is uploaded again after a lost acknowledgement.
2. Refuses a retry identity reused for a different claim.
3. Keeps two identical occurrences that have different `ps`.
4. Refuses a recorder without authority for the actor, and returns the same rejection on retry.
5. Reports exactly the `ps` gaps that were never received, and places late records correctly in occurrence order.
6. Keeps overlapping intervals on one object separate, including sub-second spans.
7. Refuses each unbalanced posting individually, including cancelling pairs in one batch and a posting that balances in USD but not in J.
8. Applies an amendment, keeps the original, traces provenance, and refuses a second amendment of a superseded posting.
9. Reprices usage under two tariffs without changing a byte of the journal.
10. Produces identical views and an identical retry index after rebuilding from the segment files alone.
11. Detects a modified byte in a sealed segment.
12. Decodes every compact block to byte-identical canonical lines, and refuses input it cannot reproduce.

## 14. Open questions for ratification

1. **Which party anchors checkpoints?** A ZAKO Tally, a signed `S checkpoint` from a second party, or both.
2. **Grant records.** Should grants be journal records (`S grant`), so that authority is replayable, or service configuration? This draft uses configuration, which means authority decisions can't yet be re-derived.
3. **Vocabulary migration.** Can a segment opened under `core@0.1` be validated under `core@0.2`? The draft says no: the rules that applied are the ones the segment declared.
4. **Task payload completeness.** Which `T.modify` fields are needed before Taskwarrior can be rebuilt rather than mirrored? This includes UDAs, dependencies, recurrence and annotations.
5. **The Workwarrior distribution-domain value** in ZAKO's 0xF0–0xFF range.
6. **Privacy.** ZAKO requires `RESTRICT_FORWARD` on JOURNAL records. Should a journal line be able to carry the same restriction, and should views and exports enforce it?
