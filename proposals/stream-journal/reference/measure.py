"""Compare encodings of the same journal records (STREAM-NOTATION draft §N8).

    python3 measure.py              # bytes, gzip, parse time
    python3 measure.py --tokens     # also Claude token counts (needs `pip install anthropic` and credentials)

Every form is checked to decode to the canonical lines before it is measured.
Token counts come from the Messages API count_tokens endpoint, which is the
only reliable source for Claude tokenization; offline tokenizers are not.
"""

from __future__ import annotations

import gzip
import json
import random
import os
import shutil
import sys
import tempfile
import timeit

from compact import context_entry, decode_pb1, encode_pb1
from notation import Notation, decode, encode
from stream_journal import Journal, Record
from test_conformance import GRANTS, T0, VOCAB, Scenario
from test_notation import cascade, journal_lines

HERE = os.path.dirname(os.path.abspath(__file__))
NT = Notation(os.path.join(HERE, "vocab", "notation-0.1.json"))
MODEL = "claude-opus-5-5"


def datasets() -> dict[str, list[str]]:
    out = {}
    d = tempfile.mkdtemp()
    try:
        Scenario(d).run()
        out["mixed scenario (35 records)"] = journal_lines(d)
    finally:
        shutil.rmtree(d)
    d = tempfile.mkdtemp()
    try:
        j = Journal(d, VOCAB, GRANTS)
        cascade(j)
        out["agent task cascade (15 records)"] = journal_lines(d)
    finally:
        shutil.rmtree(d)
    rnd = random.Random(7)  # seeded, non-periodic values 0..63 so gzip gets no free pattern
    out["meter, 1000 readings"] = [
        Record(T0 + 8 * i, "M", "measure", "pdu-4.out3",
               {"iv": 8, "ps": i + 1, "q": str(rnd.randrange(64)), "rec": "meter:pdu-4", "scope": "host:build-01", "u": "J"}).line()
        for i in range(1000)]
    return out


def forms(lines: list[str]) -> dict[str, bytes]:
    canon = ("\n".join(lines) + "\n").encode()
    run = encode(lines, NT)
    assert decode(run, NT) == lines
    f = {"canonical": canon, "canonical + gzip": gzip.compress(canon, 9),
         "run form": run.encode(), "run form + gzip": gzip.compress(run.encode(), 9)}
    recs = [Record.parse(l) for l in lines]
    if all(r.key == "M.measure" for r in recs):
        dic = [context_entry(recs[0])]
        block = encode_pb1(recs, 0, cadence=8, bits=6)
        assert [r.line() for r in decode_pb1(block, dic)] == lines
        f["PB1 binary (incl. context)"] = dic[0] + block
    return f


def parse_time(lines: list[str]) -> dict[str, float]:
    canon = "\n".join(lines) + "\n"
    run = encode(lines, NT)
    n = 20
    return {
        "canonical": timeit.timeit(lambda: [Record.parse(l) for l in canon.splitlines()], number=n) / n,
        "run form": timeit.timeit(lambda: decode(run, NT), number=n) / n,
    }


def token_counter():
    try:
        from anthropic import Anthropic
    except ImportError:
        return None
    client = Anthropic()

    def count(text: str) -> int:
        return client.messages.count_tokens(model=MODEL, messages=[{"role": "user", "content": text}]).input_tokens

    base = count(".")
    return lambda text: count(text) - base


def legend() -> str:
    with open(os.path.join(HERE, "vocab", "notation-0.1.json"), encoding="utf-8") as f:
        doc = json.load(f)
    rows = [f"{op}{g} = {k}" for k, g in doc["glyphs"].items() for op in [k.split('.')[0]]]
    return "Run-form legend\n" + "\n".join(rows) + "\nquantities: " + ", ".join(f"{k}={v['kind']}" for k, v in doc["quantities"].items())


def main():
    want_tokens = "--tokens" in sys.argv
    count = token_counter() if want_tokens else None
    if want_tokens and count is None:
        print("anthropic SDK not installed; skipping token counts\n")
    for name, lines in datasets().items():
        print(f"## {name}")
        fs = forms(lines)
        canon = len(fs["canonical"])
        print(f"{'form':30}{'bytes':>9}{'vs canonical':>14}" + (f"{'tokens':>10}" if count else ""))
        for form, data in fs.items():
            row = f"{form:30}{len(data):>9}{len(data) / canon:>13.1%} "
            if count and "gzip" not in form and "binary" not in form:
                row += f"{count(data.decode()):>9}"
            print(row)
        pt = parse_time(lines)
        print(f"parse to records (Python): canonical {pt['canonical'] * 1e3:.2f} ms, run form {pt['run form'] * 1e3:.2f} ms\n")
    if count:
        print(f"legend tokens (sent once, cacheable): {count(legend())}")


if __name__ == "__main__":
    main()
