# Proposal: STREAM Journal

**Status:** draft for review. Tracked in [issue #1](https://github.com/babbworks/ww-standard/issues/1).
**Affects:** Stream service ([Stream Event Format v1](https://github.com/babbworks/workwarrior/blob/main/dev/schemas/stream-event-v1.md)), the ledger adapter, and the Bundy and Board lenses. Stream is fragile around its log format, so this proposal includes a trial implementation, as the [proposal process](../../README.md#proposing-changes) requires.

## What this is

This proposal follows on from the STREAM / Workwarrior / ZAKO inquiry. That inquiry settled on a design: a journal with Workwarrior's readable event lines, ZAKO's vocabulary, BitPads' compact framing and BitLedger's accounting discipline. This proposal turns the design into:

- [`STREAM-JOURNAL-v0.1-DRAFT.md`](STREAM-JOURNAL-v0.1-DRAFT.md): the draft normative text.
- [`reference/`](reference/): a trial implementation and the conformance scenario, using only the Python standard library.
- [`examples/scenario/`](examples/scenario/): the journal the scenario produces, with two segments, journaled rejections and seals.
- [`STREAM-NOTATION-v0.1-DRAFT.md`](STREAM-NOTATION-v0.1-DRAFT.md): the companion design note. It covers the smallest recordable act, recording resolution in an action cascade, resource attribution, and a state-once text form (the run form) with an ASCII glyph alphabet.

## Decisions the draft takes

| Open question from the inquiry | Draft answer | § |
|---|---|---|
| Does the line format change? | No. Every journal line is a valid five-field v1 line, and all new keys go in `ctx`. | 3 |
| How are record types keyed? | By `OP.action` in a vocabulary version declared once per segment. | 4.1 |
| Should `D` become general measurement? | No. `D` stays Dey. New op codes: `M` measure, `L` ledger, `R` run span. | 4.3 |
| How are actor, recorder and signer kept apart? | `rec` is set by the journal from authentication. `actor` requires a grant when it differs from `rec`. Signatures are evidence. | 5.1 |
| What happens to a valid record that arrives late? | It is accepted. Validation is intrinsic, and views report historical contradictions. Postings and corrections are the exception. | 5.3 |
| How are retries told apart from second occurrences? | By the retry identity `(rec, ps)`. A rejection is journaled as `S reject`, so a retry gets the same answer. | 6.5 |
| How are events addressed? | `segment:seq`, implicit from position. The address is never stored on the line it names. | 2, 6 |
| What happens to ingest sort and reset? | Both go. The journal is append only, and a reset becomes a sealed segment plus a new epoch. | 6.1, 6.3 |
| How do compound groups work? | A multi-leg transaction is one record, so a batch is only transport. | 6.4 |
| What is the balance rule? | Each posting balances on its own, per (book, unit). | 8.2 |
| Which ZAKO verb represents a punch-out? | None fits. Carry it in a Workwarrior distribution domain (0xF0–0xFF), the extension path in ZAKO Codebook §8.1. | 11.2 |

## Trial results

Run the trial with:

```bash
cd reference
python3 -m unittest -v test_conformance                                 # 20 tests
WW_REPO=/path/to/workwarrior python3 -m unittest -v test_conformance    # +3 against shipped v1 code
```

The suite ran on 2026-11-22 against Workwarrior `ce7379f`. **All 23 tests passed.** Three of them ran the shipped v1 code: the Bundy lens and `_dedup_events`.

**What v1 does with the same facts.** Operator 3 worked lathe-01 for 20 minutes and operator 7 for 40 minutes, with the two shifts overlapping. The shipped Bundy lens reports **one interval of 10 minutes**. In v1 ingest, a third operator's punch-in at the same second as another punch on the same machine is **silently dropped**. The journal keeps all three operators, reports both completed intervals correctly, and lists the third operator's unclosed punch as open rather than closing it.

**Compact encoding**, with each decode checked against the canonical lines byte for byte:

| Batch | Canonical lines | PB1 |
|---|---|---|
| 3 readings (38, 41, 36 J every 8 s) | 327 B | 3 B payload, 26 B block, plus a 115 B context entry sent once |
| 1,000 readings, 6-bit values | 110,737 B | 750 B payload, 773 B block, plus the 115 B context entry |

These canonical lines carry `rec`, `ps`, `scope` and `iv`, so they are larger than the inquiry's illustrative 294 B lines. For a small batch, the context entry is most of the cost. PB1 starts to pay once a context is reused across blocks or the batch is long.

**Design gap found by the trial.** The first run showed that rejected records consumed producer sequence numbers without leaving a trace, so the gap view could not tell "rejected" from "never received". The draft now journals rejections (§6.5), and capture completeness reads them (§6.7).

### Notation trial

`python3 -m unittest -v test_notation` runs 11 more tests, and `python3 measure.py` reproduces the size table. Every form decodes to the canonical lines byte for byte.

| Data | Canonical | + gzip | Run form | Run form + gzip | PB1 |
|---|---|---|---|---|---|
| Mixed journal (35 records) | 4,618 B | 1,393 B | 2,537 B | 1,192 B | — |
| Agent task cascade (15 records) | 2,147 B | 457 B | 694 B | 309 B | — |
| Meter, 1,000 readings | 110,728 B | 7,514 B | 3,923 B | 1,342 B | 888 B |

Token counts aren't measured yet. `python3 measure.py --tokens` measures them with the Messages API `count_tokens` endpoint, given the `anthropic` package and credentials.

## What the trial does not show

- It runs one process and one stream, and there is no network. Durability is a single `fsync` per line.
- Grants are configuration rather than journal records (open question 2).
- It doesn't anchor checkpoints externally, handle signed BitPads frames, or test BitPads interop.
- The task view folds only `add`, `modify`, `start`, `done` and `delete`. That falls short of a Taskwarrior replacement (open question 4).
- Epochs (§6.3) and `L.reverse` are specified but not exercised by the scenario.
- Throughput is untested.

## Suggested next steps

1. Review the draft decisions above, especially §5.3 (accept late records, report contradictions) and §6.5 (journaled rejections).
2. Discuss the draft in [issue #1](https://github.com/babbworks/ww-standard/issues/1).
3. If accepted, write Workwarrior task cards for:
   - pairing on `run` or `(object, actor)` in Bundy and Board, a change that is independent of everything else and already fixes the overlap bug;
   - replacing dedup with retry identity;
   - removing the ingest sort;
   - adding `L` legs to the ledger adapter.
4. Raise §11.2 (the distribution domain) and §11.4 (the divergences) with the ZAKO Codebook owners.
