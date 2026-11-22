"""Conformance trial for the STREAM Journal draft (§13).

Run from this directory:

    python3 -m unittest -v test_conformance

Set WW_REPO=/path/to/workwarrior to also run the legacy comparison against
the real Stream v1 code (Bundy lens and ingest dedup).
"""

from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
import unittest

from compact import context_entry, decode_pb1, encode_pb1
from stream_journal import (Grants, Journal, Record, Vocab, balances, capture_gaps, intervals, price,
                            provenance, snapshot, tasks, timeline)

HERE = os.path.dirname(os.path.abspath(__file__))
VOCAB = Vocab(os.path.join(HERE, "vocab", "core-0.1.json"))
GRANTS = Grants({
    "host:build-01": ["agent:*"],          # the host records what its agents do
    "human:acct-1": ["org:*"],             # the accountant posts for the organisation
    "import:ww-v1": ["*"],                 # legacy import may carry any actor, see §12
})
T0 = 1752300000


def line(ts, op, action, obj, **ctx) -> str:
    return Record(ts, op, action, obj, ctx).line()


def legs(*triples, book=None):
    out = []
    for a, q, u in triples:
        leg = {"a": a, "q": q, "u": u}
        if book:
            leg["book"] = book
        out.append(leg)
    return out


class Scenario:
    """The reconstruction-after-failure scenario. Every step keeps its receipts."""

    def __init__(self, path: str):
        self.j = Journal(path, VOCAB, GRANTS)
        self.r: dict[str, list] = {}

    def run(self):
        j, r = self.j, self.r

        # 1. Overlapping punches on one machine by two operators.
        r["op07"] = j.submit("human:op-07", [
            line(T0 + 600, "B", "start", "lathe-01", ps=1, proj="line-a"),
            line(T0 + 3000, "B", "stop", "lathe-01", ps=2, proj="line-a"),
        ], received_at=T0 + 3001)

        # 2. An agent run reported by its host: actor != recorder. Sub-second
        #    timing in tf; model and tool spans overlap inside the session.
        r["agent"] = j.submit("host:build-01", [
            line(T0 + 100, "R", "open", "run-r1", actor="agent:claude-07", run="r1", span="session", ps=1),
            line(T0 + 101, "R", "open", "run-r1.m1", actor="agent:claude-07", run="r1.m1", parent="r1", span="model", tf=250000, ps=2),
            line(T0 + 102, "R", "open", "run-r1.t1", actor="agent:claude-07", run="r1.t1", parent="r1", span="tool", tf=0, ps=3),
            line(T0 + 103, "R", "close", "run-r1.m1", actor="agent:claude-07", run="r1.m1", status="ok", tf=750000, ps=4),
            line(T0 + 104, "M", "measure", "run-r1.m1", actor="agent:claude-07", q="1200", u="tok.in", provider="anthropic", model="resolved-model-id", ps=5),
            line(T0 + 104, "M", "measure", "run-r1.m1", actor="agent:claude-07", q="350", u="tok.out", provider="anthropic", model="resolved-model-id", ps=6),
            line(T0 + 110, "R", "close", "run-r1.t1", actor="agent:claude-07", run="r1.t1", status="fail", ps=7),
            line(T0 + 120, "R", "close", "run-r1", actor="agent:claude-07", run="r1", status="ok", ps=8),
        ], received_at=T0 + 121)

        # 3. Host energy meter: sample 3 never arrives (gap); sample 5 arrives
        #    late, after 6, with an earlier occurrence time.
        meter = lambda ts, ps, q: line(ts, "M", "measure", "pdu-4.out3", q=q, u="J", iv=8, scope="host:build-01", ps=ps)
        r["meter"] = j.submit("meter:pdu-4", [
            meter(T0 + 96, 1, "38"), meter(T0 + 104, 2, "41"), meter(T0 + 120, 4, "36"), meter(T0 + 136, 6, "36"),
        ], received_at=T0 + 140)
        r["meter_late"] = j.submit("meter:pdu-4", [meter(T0 + 128, 5, "36")], received_at=T0 + 400)

        # 4. Allocation of host energy to the agent is an estimate with a method.
        basis = [x.addr for x in r["meter"][:2]]
        r["alloc"] = j.submit("host:build-01", [
            line(T0 + 120, "M", "estimate", "run-r1", actor="agent:claude-07", q="52.4", u="J",
                 method="cpu-share@1", basis=basis, ps=9),
        ], received_at=T0 + 401)

        # 5. Operator 3 was offline. Their spool uploads; the ack is lost; the
        #    spool uploads again. Same receipts, no new records.
        spool = [
            line(T0 + 0, "B", "start", "lathe-01", ps=1, proj="line-a"),
            line(T0 + 1200, "B", "stop", "lathe-01", ps=2, proj="line-a"),
            line(T0 + 1300, "T", "add", "wo-8891", desc="Turn shaft batch 12", ps=3),
            line(T0 + 1310, "T", "start", "wo-8891", ps=4),
        ]
        r["spool"] = j.submit("human:op-03", spool, received_at=T0 + 5000)
        r["spool_retry"] = j.submit("human:op-03", spool, received_at=T0 + 5060)

        # 6. Same retry id, different claim: refused, original untouched.
        r["conflict"] = j.submit("human:op-03", [line(T0 + 1200, "B", "stop", "lathe-01", ps=2, proj="line-b")], received_at=T0 + 5061)

        # 7. Two identical readings that really happened twice: different ps.
        r["twice"] = j.submit("scale:s1", [
            line(T0 + 2000, "M", "measure", "bin-7", q="92", u="kg", ps=1),
            line(T0 + 2000, "M", "measure", "bin-7", q="92", u="kg", ps=2),
        ], received_at=T0 + 2001)

        # 8. Authority: op-07 cannot punch for op-03.
        r["forged"] = j.submit("human:op-07", [line(T0 + 4000, "B", "start", "lathe-01", actor="human:op-03", ps=3)], received_at=T0 + 4001)

        # 9. Accounting. Inventory posting; an upload containing two
        #    transactions whose errors cancel; a mixed-unit transaction.
        r["inventory"] = j.submit("human:acct-1", [
            line(T0 + 2100, "L", "post", "run-r1", actor="org:plant", ps=1, legs=legs(
                ("inv:raw", "-100", "kg"), ("inv:product", "92", "kg"), ("inv:scrap", "8", "kg"))),
        ], received_at=T0 + 2101)
        r["cancelling"] = j.submit("human:acct-1", [
            line(T0 + 2200, "L", "post", "x", actor="org:plant", ps=2, legs=legs(("a", "10", "USD"), ("b", "-5", "USD"))),
            line(T0 + 2201, "L", "post", "x", actor="org:plant", ps=3, legs=legs(("a", "-10", "USD"), ("b", "5", "USD"), ("c", "0.00", "USD"))),
            line(T0 + 2202, "L", "post", "x", actor="org:plant", ps=4, legs=legs(("a", "-5", "USD"), ("b", "10", "USD"))),
        ], received_at=T0 + 2203)
        r["mixed"] = j.submit("human:acct-1", [
            line(T0 + 2300, "L", "post", "run-r1", actor="org:plant", ps=5, legs=legs(
                ("energy:host", "-52.4", "J"), ("energy:run-r1", "52.4", "J"),
                ("cost:compute", "0.019", "USD"), ("ap:provider", "-0.019", "USD"))),
            line(T0 + 2301, "L", "post", "run-r1", actor="org:plant", ps=6, legs=legs(
                ("energy:host", "-52.4", "J"), ("energy:run-r1", "52.0", "J"),
                ("cost:compute", "0.019", "USD"), ("ap:provider", "-0.019", "USD"))),
        ], received_at=T0 + 2302)

        # 10. Verification finds 91 kg product, 9 kg scrap. Amend, never erase.
        orig = r["inventory"][0].addr
        r["amend"] = j.submit("human:acct-1", [
            line(T0 + 2400, "L", "amend", "run-r1", actor="org:plant", ps=7, ref=orig, reason="weighbridge recheck",
                 legs=legs(("inv:raw", "-100", "kg"), ("inv:product", "91", "kg"), ("inv:scrap", "9", "kg"))),
        ], received_at=T0 + 2401)
        r["double_amend"] = j.submit("human:acct-1", [
            line(T0 + 2500, "L", "amend", "run-r1", actor="org:plant", ps=8, ref=orig, reason="again",
                 legs=legs(("inv:raw", "-100", "kg"), ("inv:product", "90", "kg"), ("inv:scrap", "10", "kg"))),
        ], received_at=T0 + 2501)

        # 11. Seal the segment, keep writing in the next one.
        r["seal1"] = j.seal(T0 + 6000)
        r["after_seal"] = j.submit("human:op-03", [line(T0 + 6100, "T", "done", "wo-8891", ps=5)], received_at=T0 + 6101)
        return self


class Conformance(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp(prefix="stream-journal-")
        self.s = Scenario(self.dir).run()
        self.j, self.r = self.s.j, self.s.r

    def tearDown(self):
        shutil.rmtree(self.dir, ignore_errors=True)

    # --- capture --------------------------------------------------------

    def test_lost_ack_retry_returns_original_receipts(self):
        first, second = self.r["spool"], self.r["spool_retry"]
        self.assertTrue(all(x.status == "accepted" for x in first))
        self.assertEqual([x.status for x in second], ["duplicate"] * 4)
        self.assertEqual([x.addr for x in first], [x.addr for x in second])

    def test_conflicting_retry_is_refused(self):
        self.assertEqual(self.r["conflict"][0].status, "rejected")
        self.assertIn("already used by a different claim", self.r["conflict"][0].reason)

    def test_identical_second_occurrence_is_kept(self):
        self.assertEqual([x.status for x in self.r["twice"]], ["accepted", "accepted"])

    def test_recorder_needs_authority_for_actor(self):
        forged = self.r["forged"][0]
        self.assertEqual(forged.status, "rejected")
        # The rejection is journaled; retrying the same claim gets the same answer.
        retry = self.j.submit("human:op-07", [line(T0 + 4000, "B", "start", "lathe-01", actor="human:op-03", ps=3)], T0 + 4100)[0]
        self.assertEqual((retry.status, retry.addr, retry.reason), (forged.status, forged.addr, forged.reason))
        agent = self.j.by_addr[next(a for a, rec in self.j.records if rec.obj == "run-r1" and rec.key == "R.open")]
        self.assertEqual((agent.ctx["actor"], agent.ctx["rec"]), ("agent:claude-07", "host:build-01"))

    def test_gap_and_late_arrival(self):
        self.assertEqual(capture_gaps(self.j), {"meter:pdu-4": [3]})
        late = self.r["meter_late"][0].addr
        tl = timeline(self.j)
        meter = [a for a in tl if self.j.by_addr[_A(a)].obj == "pdu-4.out3"]
        # Arrived last, sits between samples 4 and 6 in occurrence order.
        self.assertEqual([self.j.by_addr[_A(a)].ctx["ps"] for a in meter], [1, 2, 4, 5, 6])
        six = next(a for a in meter if self.j.by_addr[_A(a)].ctx["ps"] == 6)
        self.assertGreater(_A(late).seq, _A(six).seq)  # ...but was committed after sample 6

    # --- intervals ------------------------------------------------------

    def test_overlapping_punches_survive(self):
        b = intervals(self.j, "B")
        got = sorted((i["actor"], i["dur_s"]) for i in b["closed"])
        self.assertEqual(got, [("human:op-03", 1200.0), ("human:op-07", 2400.0)])
        self.assertEqual(b["anomalies"], [])

    def test_run_tree_with_subsecond_overlap(self):
        runs = {i["key"][0]: i for i in intervals(self.j, "R")["closed"]}
        self.assertEqual(runs["r1.m1"]["dur_s"], 2.5)
        self.assertEqual(runs["r1.t1"]["status"], "fail")
        self.assertEqual(runs["r1.t1"]["parent"], "r1")
        self.assertEqual(runs["r1"]["dur_s"], 20.0)

    # --- accounting -----------------------------------------------------

    def test_each_transaction_balances_on_its_own(self):
        c = self.r["cancelling"]
        self.assertEqual([x.status for x in c], ["rejected", "rejected", "rejected"])
        self.assertIn("unbalanced", c[0].reason)
        self.assertIn("zero quantity", c[1].reason)

    def test_balance_is_per_unit(self):
        self.assertEqual(self.r["mixed"][0].status, "accepted")
        self.assertEqual(self.r["mixed"][1].status, "rejected")
        self.assertIn("-/J", self.r["mixed"][1].reason)

    def test_amendment_preserves_evidence(self):
        b = balances(self.j)
        self.assertEqual(b["-|inv:product|kg"]["q"], "91")
        self.assertEqual(b["-|inv:scrap|kg"]["q"], "9")
        amend = self.r["amend"][0].addr
        self.assertEqual(provenance(self.j, amend), [amend, self.r["inventory"][0].addr])
        self.assertEqual(self.r["double_amend"][0].status, "rejected")
        # The original claim is still in the journal, unchanged.
        orig = self.j.by_addr[_A(self.r["inventory"][0].addr)]
        self.assertEqual(orig.ctx["legs"][1]["q"], "92")

    def test_repricing_does_not_touch_history(self):
        before = _read(os.path.join(self.dir, "seg-000001.log"))
        v1 = price(self.j, {"version": "t1", "rates": {"tok.in": "0.000003", "tok.out": "0.000015"}})
        v2 = price(self.j, {"version": "t2", "rates": {"tok.in": "0.0000025", "tok.out": "0.0000125"}})
        self.assertEqual(v1, {"run-r1.m1": "0.008850"})
        self.assertEqual(v2, {"run-r1.m1": "0.0073750"})
        self.assertEqual(before, _read(os.path.join(self.dir, "seg-000001.log")))

    # --- tasks ----------------------------------------------------------

    def test_task_state_across_segments(self):
        t = tasks(self.j)["wo-8891"]
        self.assertEqual(t["status"], "completed")
        self.assertEqual(t["history"][-1].split(":")[0], "2")

    # --- journal --------------------------------------------------------

    def test_rebuild_from_segments_alone(self):
        before = snapshot(self.j)
        fresh = Journal(self.dir, VOCAB, GRANTS)
        self.assertEqual(snapshot(fresh), before)
        # The retry index is rebuilt too: the spool is still a duplicate.
        again = fresh.submit("human:op-03", [line(T0 + 0, "B", "start", "lathe-01", ps=1, proj="line-a")], received_at=T0 + 9000)
        self.assertEqual((again[0].status, again[0].addr), ("duplicate", self.r["spool"][0].addr))

    def test_seal_chain_detects_tampering(self):
        self.assertEqual(self.j.verify(), [])
        p = os.path.join(self.dir, "seg-000001.log")
        _write(p, _read(p).replace(b'"q":"92"', b'"q":"93"', 1))
        self.assertIn("seg 1: body hash mismatch", Journal(self.dir, VOCAB, GRANTS).verify())

    def test_lines_remain_stream_v1(self):
        # Every committed line parses with the v1 maxsplit=4 rule and has JSON ctx.
        for n in self.j.segments():
            for raw in _read(os.path.join(self.dir, f"seg-{n:06d}.log")).decode().splitlines():
                parts = raw.split(None, 4)
                self.assertEqual(len(parts), 5)
                self.assertTrue(parts[0].isdigit())


class Compact(unittest.TestCase):
    def _readings(self, values, t0=T0, cad=8, skip=()):
        out, ps = [], 1
        for i, v in enumerate(values):
            if i in skip:
                continue
            out.append(Record(t0 + i * cad, "M", "measure", "pdu-4.out3",
                              {"iv": cad, "ps": ps, "q": v, "rec": "meter:pdu-4", "scope": "host:build-01", "u": "J"}))
            ps += 1
        return out

    def _roundtrip(self, recs, **kw):
        dictionary = [context_entry(recs[0])]
        block = encode_pb1(recs, 0, **kw)
        self.assertEqual([r.line() for r in decode_pb1(block, dictionary)], [r.line() for r in recs])
        return block, dictionary

    def test_three_readings(self):
        recs = self._readings(["38", "41", "36"])
        block, d = self._roundtrip(recs, cadence=8, bits=6)
        self.assertEqual(len(block), 23 + 3)
        print(f"\n  3 readings: canonical {sum(len(r.line()) + 1 for r in recs)} B; "
              f"PB1 payload 3 B, block {len(block)} B, + context entry {len(d[0])} B once")

    def test_thousand_readings_six_bits(self):
        vals = [str((i * 37) % 64) for i in range(1000)]
        recs = self._readings(vals)
        block, d = self._roundtrip(recs, cadence=8, bits=6)
        self.assertEqual(len(block) - 23, 750)
        print(f"\n  1000 readings: canonical {sum(len(r.line()) + 1 for r in recs)} B; "
              f"PB1 {len(block)} B + {len(d[0])} B context")

    def test_exceptions_roundtrip(self):
        recs = self._readings(["38", "41", "80", "36", "-2"], skip={3})
        block, _ = self._roundtrip(recs, cadence=8, bits=6)
        self.assertEqual(len(block), 23 + 2 * 7 + 3 + 4)  # header, 2 overrides, 1 missing, 5 slots x 6 bits

    def test_decimal_exponent(self):
        recs = self._readings(["3.8", "4.1", "3.6"])
        self._roundtrip(recs, cadence=8, bits=6, exp=-1)

    def test_refuses_what_it_cannot_reproduce(self):
        from compact import NotEncodable
        with self.assertRaises(NotEncodable):
            encode_pb1(self._readings(["38.0"]), 0, cadence=8, bits=6)  # would decode as "38"
        off = self._readings(["1", "2"])
        off[1] = Record(off[1].ts + 1, *[getattr(off[1], f) for f in ("op", "action", "obj", "ctx")])
        with self.assertRaises(NotEncodable):
            encode_pb1(off, 0, cadence=8, bits=6)


@unittest.skipUnless(os.environ.get("WW_REPO"), "set WW_REPO to compare with Stream v1")
class LegacyComparison(unittest.TestCase):
    """The same facts through the shipped Stream v1 code, for contrast (§12)."""

    LINES = [
        '1752300000 B start lathe-01 {"actor":"human:op-03","proj":"line-a"}',
        '1752300600 B start lathe-01 {"actor":"human:op-07","proj":"line-a"}',
        '1752301200 B stop lathe-01 {"actor":"human:op-03","proj":"line-a"}',
        '1752303000 B stop lathe-01 {"actor":"human:op-07","proj":"line-a"}',
    ]

    def _sh(self, script, stdin):
        stream = os.path.join(os.environ["WW_REPO"], "services", "stream")
        return subprocess.run(["bash", "-c", script], input=stdin, capture_output=True, text=True, cwd=stream).stdout

    def test_v1_bundy_merges_overlap(self):
        out = self._sh("source lenses/bundy.sh; lens_run", "\n".join(self.LINES) + "\n")
        row = next(l for l in out.splitlines() if l.startswith("line-a "))
        self.assertEqual(row.split()[1:4], ["1", "0h", "10m"])  # v1: one 10-minute interval

    def test_v1_dedup_drops_distinct_actor(self):
        d = tempfile.mkdtemp()
        try:
            log = os.path.join(d, "stream.log")
            _write(log, ("\n".join(self.LINES) + "\n").encode())
            new = '1752300000 B start lathe-01 {"actor":"human:op-09","proj":"line-a"}\n'
            out = self._sh(f"source lib/adapters.sh; _dedup_events {log}", new)
            self.assertEqual(out, "")  # v1: op-09's start is silently dropped
        finally:
            shutil.rmtree(d)

    def test_journal_keeps_both(self):
        d = tempfile.mkdtemp()
        try:
            j = Journal(d, VOCAB, GRANTS)
            lines = self.LINES + ['1752300000 B start lathe-01 {"actor":"human:op-09","proj":"line-a"}']
            # Legacy lines carry no ps; the importer assigns rk = digest(line) + occurrence (§12.2).
            seen, batch = {}, []
            for l in lines:
                n = seen[l] = seen.get(l, 0) + 1
                rec = Record.parse(l)
                rk = f"{__import__('hashlib').sha256(l.encode()).hexdigest()[:16]}.{n}"
                batch.append(Record(rec.ts, rec.op, rec.action, rec.obj, {**rec.ctx, "rk": rk}).line())
            self.assertEqual([x.status for x in j.submit("import:ww-v1", batch, T0 + 9000)], ["accepted"] * 5)
            b = intervals(j, "B")
            self.assertEqual(sorted((i["actor"], i["dur_s"]) for i in b["closed"]),
                             [("human:op-03", 1200.0), ("human:op-07", 2400.0)])
            self.assertEqual(len(b["open"]), 1)  # op-09 punched in and never out: surfaced, not invented
        finally:
            shutil.rmtree(d)


def _read(p):
    with open(p, "rb") as f:
        return f.read()


def _write(p, b):
    with open(p, "wb") as f:
        f.write(b)


def _A(s):
    from stream_journal import Addr
    return Addr.parse(s)


if __name__ == "__main__":
    unittest.main()
