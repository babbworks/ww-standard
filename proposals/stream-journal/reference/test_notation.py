"""Run-form notation and resource attribution (STREAM-NOTATION draft).

    python3 -m unittest -v test_notation
"""

from __future__ import annotations

import os
import shutil
import tempfile
import unittest

from notation import Notation, decode, encode
from stream_journal import Grants, Journal, Record, Vocab, attribution
from test_conformance import GRANTS, T0, VOCAB, Scenario, line

HERE = os.path.dirname(os.path.abspath(__file__))
NT = Notation(os.path.join(HERE, "vocab", "notation-0.1.json"))
CONSERVED = [k for k, v in NT.quantities.items() if v["conserved"]]


def journal_lines(path: str) -> list[str]:
    out = []
    for f in sorted(os.listdir(path)):
        if f.startswith("seg-"):
            with open(os.path.join(path, f), encoding="utf-8") as fh:
                out += [l.rstrip("\n") for l in fh]
    return out


def cascade(j: Journal):
    """One agent task. Host meter totals at each level; children below 'process' not recorded."""
    a = dict(actor="agent:claude-07")
    return j.submit("host:build-01", [
        line(T0 + 0,  "R", "open",  "task-42", run="k42", span="task", ps=1, **a),
        line(T0 + 0,  "R", "open",  "task-42", run="k42.m1", parent="k42", span="model", tf=100000, ps=2, **a),
        line(T0 + 4,  "R", "close", "task-42", run="k42.m1", status="ok", tf=300000, ps=3,
             use={"ti": "12000", "to": "900"}, **a),
        line(T0 + 4,  "R", "open",  "task-42", run="k42.t1", parent="k42", span="tool", tf=500000, ps=4, **a),
        line(T0 + 4,  "R", "open",  "task-42", run="k42.t1.p1", parent="k42.t1", span="process", tf=600000, ps=5, **a),
        line(T0 + 39, "R", "close", "task-42", run="k42.t1.p1", status="ok", ps=6,
             use={"xs": "32.0", "J": "~470", "B": "51000000"}, **a),
        line(T0 + 40, "R", "close", "task-42", run="k42.t1", status="ok", ps=7,
             use={"xs": "33.1", "J": "~480", "B": "52000000", "#": "412"}, **a),
        line(T0 + 41, "R", "open",  "task-42", run="k42.m2", parent="k42", span="model", ps=8, **a),
        line(T0 + 44, "R", "close", "task-42", run="k42.m2", status="ok", ps=9,
             use={"ti": "6000", "to": "1500"}, **a),
        line(T0 + 45, "R", "open",  "task-42", run="k42.t2", parent="k42", span="tool", ps=10, **a),
        line(T0 + 45, "R", "open",  "task-42", run="k42.t3", parent="k42", span="tool", ps=11, **a),
        line(T0 + 53, "R", "close", "task-42", run="k42.t2", status="ok", ps=12, use={"xs": "3.0", "J": "~40"}, **a),
        line(T0 + 55, "R", "close", "task-42", run="k42.t3", status="fail", ps=13, use={"xs": "2.4", "J": "~35"}, **a),
        line(T0 + 60, "R", "close", "task-42", run="k42", status="ok", ps=14,
             use={"xs": "41.0", "J": "610", "ti": "18000", "to": "2400"}, **a),
    ], received_at=T0 + 61)


class RunForm(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp(prefix="notation-")

    def tearDown(self):
        shutil.rmtree(self.dir, ignore_errors=True)

    def assertRoundTrip(self, lines):
        text = encode(lines, NT)
        self.assertEqual(decode(text, NT), lines)
        return text

    def test_scenario_journal_round_trips(self):
        Scenario(self.dir).run()
        lines = journal_lines(self.dir)
        text = self.assertRoundTrip(lines)
        canon = sum(len(l) + 1 for l in lines)
        print(f"\n  scenario: {len(lines)} records, canonical {canon} B, run form {len(text.encode())} B")
        self.assertLess(len(text.encode()), canon * 0.6)

    def test_periodic_readings_use_a_template(self):
        recs = [Record(T0 + 8 * i, "M", "measure", "pdu-4.out3",
                       {"iv": 8, "ps": i + 1, "q": str((i * 37) % 64), "rec": "meter:pdu-4", "scope": "host:build-01", "u": "J"})
                for i in range(1000)]
        lines = [r.line() for r in recs]
        text = self.assertRoundTrip(lines)
        self.assertTrue(text.startswith(f"@{T0} %dt=8 %op=M= %obj=pdu-4.out3 %ps=1 "))
        self.assertEqual(text.splitlines()[1], "0J")
        print(f"\n  1000 readings: canonical {sum(len(l) + 1 for l in lines)} B, run form {len(text.encode())} B")

    def test_three_readings_text(self):
        recs = [Record(T0 + 8 * i, "M", "measure", "pdu-4.out3",
                       {"iv": 8, "ps": i + 1, "q": q, "rec": "meter:pdu-4", "scope": "host:build-01", "u": "J"})
                for i, q in enumerate(["38", "41", "36"])]
        text = self.assertRoundTrip([r.line() for r in recs])
        self.assertEqual(text.splitlines()[1:], ["38J", "41J", "36J"])

    def test_gap_in_cadence_is_marked(self):
        recs = [Record(T0 + 8 * i, "M", "measure", "m", {"ps": n, "q": "1", "u": "J"})
                for n, i in enumerate([0, 1, 3, 4], start=1)]
        text = self.assertRoundTrip([r.line() for r in recs])
        self.assertIn("\n_\n", text)

    def test_unglyphed_actions_and_awkward_values_survive(self):
        lines = [
            line(T0, "T", "track", "x", note="has spaces", tags=["a", "b"]),
            line(T0 + 1, "M", "measure", "m", q="5", u="J", label="123"),       # string that looks numeric
            line(T0 + 2, "M", "measure", "m", q="5.0", u="J"),
            line(T0 + 2, "L", "post", "p", legs=[{"a": "a=b", "q": "1", "u": "J"}, {"a": "c", "q": "-1", "u": "J"}]),
            line(T0 + 3, "A", "note", "y", text='quote " and {brace}'),
            line(T0 - 30, "T", "done", "x", tf=0),                               # late, sub-second, negative delta
        ]
        self.assertRoundTrip(lines)


class Attribution(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp(prefix="cascade-")
        self.j = Journal(self.dir, VOCAB, GRANTS)
        self.receipts = cascade(self.j)

    def tearDown(self):
        shutil.rmtree(self.dir, ignore_errors=True)

    def test_accepted(self):
        self.assertEqual({r.status for r in self.receipts}, {"accepted"}, [r.reason for r in self.receipts])

    def test_remainder_is_work_below_recorded_resolution(self):
        a = attribution(self.j, CONSERVED)
        top = a["k42"]
        self.assertEqual(top["remainder"]["xs"], "2.5")      # 41.0 - (33.1 + 3.0 + 2.4)
        self.assertEqual(top["remainder"]["J"], "~55")       # estimates propagate
        self.assertEqual(top["remainder"]["ti"], "0")        # every input token is accounted for
        self.assertEqual(top["remainder"]["to"], "0")
        tool = a["k42.t1"]
        self.assertEqual(tool["remainder"], {"xs": "1.1", "J": "~10", "B": "1000000", "#": "412"})
        self.assertEqual(top["over"], {})

    def test_wall_time_is_an_envelope_not_a_sum(self):
        a = attribution(self.j, CONSERVED)
        self.assertEqual(a["k42"]["wall_s"], 60.0)
        self.assertEqual(a["k42"]["utilisation"], 0.683)     # 41 cpu-s in 60 s
        self.assertEqual(a["k42.t1"]["parallelism"], 0.969)  # one process, 34.4 s of a 35.5 s tool call
        # t2 and t3 overlap: their wall times add to more than the 10 s they spanned.
        kids = sum(a[k]["wall_s"] for k in ("k42.t2", "k42.t3"))
        self.assertEqual(kids, 18.0)

    def test_over_attribution_is_reported(self):
        r = self.j.submit("host:build-01", [
            line(T0 + 100, "R", "open", "t", run="z", span="task", ps=15, actor="agent:claude-07"),
            line(T0 + 100, "R", "open", "t", run="z.1", parent="z", span="tool", ps=16, actor="agent:claude-07"),
            line(T0 + 101, "R", "close", "t", run="z.1", status="ok", use={"J": "9"}, ps=17, actor="agent:claude-07"),
            line(T0 + 102, "R", "close", "t", run="z", status="ok", use={"J": "5"}, ps=18, actor="agent:claude-07"),
        ], received_at=T0 + 103)
        self.assertEqual({x.status for x in r}, {"accepted"})
        self.assertEqual(attribution(self.j, CONSERVED)["z"]["over"], {"J": "-4"})

    def test_bad_use_value_rejected(self):
        r = self.j.submit("host:build-01", [line(T0 + 200, "R", "close", "t", run="k42", status="ok",
                                                 use={"J": 5}, ps=19, actor="agent:claude-07")], received_at=T0 + 201)
        self.assertEqual(r[0].status, "rejected")

    def test_cascade_in_run_form(self):
        lines = journal_lines(self.dir)
        text = encode(lines, NT)
        self.assertEqual(decode(text, NT), lines)
        print("\n" + "\n".join("    " + l for l in text.splitlines()))
        print(f"  cascade: canonical {sum(len(l) + 1 for l in lines)} B, run form {len(text.encode())} B")


if __name__ == "__main__":
    unittest.main()
