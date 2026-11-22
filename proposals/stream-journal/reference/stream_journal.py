"""STREAM Journal — reference implementation (trial, stdlib only).

This is a *trial implementation* of the draft in ../STREAM-JOURNAL-v0.1-DRAFT.md.
It exists to make the draft's guarantees testable, not to be deployed. Every
section number in a comment refers to that draft.

Layout on disk (one stream):

    <dir>/seg-000001.log      five-field Stream v1 lines, append-only
    <dir>/seg-000002.log      ...

Line 1 of every segment is `S open`; the last line of a sealed segment is
`S seal`. Nothing else is stored. The retry index, producer-sequence state and
every view are rebuilt from the segments (§6.6).
"""

from __future__ import annotations

import fnmatch
import hashlib
import json
import os
from dataclasses import dataclass, field
from decimal import Decimal, InvalidOperation
from typing import Iterable

GENESIS = "0" * 64


# --------------------------------------------------------------------------
# Records (§3)
# --------------------------------------------------------------------------

def canon_ctx(ctx: dict) -> str:
    """Canonical ctx serialisation: sorted keys, no whitespace (§3.3)."""
    return json.dumps(ctx, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


@dataclass(frozen=True)
class Record:
    ts: int
    op: str
    action: str
    obj: str
    ctx: dict

    @property
    def key(self) -> str:
        return f"{self.op}.{self.action}"

    def line(self) -> str:
        return f"{self.ts} {self.op} {self.action} {self.obj} {canon_ctx(self.ctx)}"

    @staticmethod
    def parse(line: str) -> "Record":
        parts = line.rstrip("\n").split(None, 4)
        if len(parts) < 4:
            raise ValueError("fewer than four positional fields")
        ctx = json.loads(parts[4]) if len(parts) == 5 else {}
        if not isinstance(ctx, dict):
            raise ValueError("ctx is not a JSON object")
        return Record(int(parts[0]), parts[1], parts[2], parts[3], ctx)

    def actor(self) -> str:
        # §4.1: actor falls back to recorder, then to the v1 profile fallback.
        return self.ctx.get("actor") or self.ctx.get("rec") or "profile:" + self.ctx.get("prof", "default")


@dataclass(frozen=True)
class Addr:
    seg: int
    seq: int

    def __str__(self) -> str:
        return f"{self.seg}:{self.seq}"

    @staticmethod
    def parse(s: str) -> "Addr":
        a, b = s.split(":")
        return Addr(int(a), int(b))


# --------------------------------------------------------------------------
# Vocabulary and authority (§4, §5.2)
# --------------------------------------------------------------------------

class Vocab:
    def __init__(self, path: str):
        with open(path, encoding="utf-8") as f:
            doc = json.load(f)
        self.version: str = doc["vocab"]
        self.defs: dict = doc["definitions"]

    def get(self, key: str) -> dict | None:
        return self.defs.get(key)


class Grants:
    """recorder -> actor patterns it may record on behalf of (§5.2).

    A recorder may always record for itself. Everything else must be granted.
    """

    def __init__(self, grants: dict[str, list[str]] | None = None):
        self.grants = grants or {}

    def allows(self, rec: str, actor: str) -> bool:
        if actor == rec:
            return True
        return any(fnmatch.fnmatchcase(actor, p) for p in self.grants.get(rec, []))


# --------------------------------------------------------------------------
# Receipts (§6.5)
# --------------------------------------------------------------------------

@dataclass(frozen=True)
class Receipt:
    status: str            # accepted | duplicate | rejected
    addr: str | None = None
    reason: str | None = None


class Rejected(Exception):
    pass


def _dec(v, what: str) -> Decimal:
    if not isinstance(v, str):
        raise Rejected(f"{what} must be a decimal string, got {type(v).__name__}")
    try:
        d = Decimal(v)
    except InvalidOperation:
        raise Rejected(f"{what} is not a decimal: {v!r}")
    if not d.is_finite():
        raise Rejected(f"{what} is not finite")
    return d


def _digest(b: bytes) -> str:
    return hashlib.sha256(b).hexdigest()


# --------------------------------------------------------------------------
# Journal (§6)
# --------------------------------------------------------------------------

class Journal:
    def __init__(self, path: str, vocab: Vocab, grants: Grants, stream: str = "main"):
        self.path = path
        self.vocab = vocab
        self.grants = grants
        self.stream = stream
        os.makedirs(path, exist_ok=True)
        # Everything below is derived state, rebuilt by _load().
        self.records: list[tuple[Addr, Record]] = []
        self.by_addr: dict[Addr, Record] = {}
        # (rec, retry-id) -> (addr, claim digest, rejection reason or None)
        self.retry: dict[tuple[str, str], tuple[Addr, str, str | None]] = {}
        self.superseded: dict[Addr, Addr] = {}                       # posting -> correction
        self.seg = 0
        self.seq = 0
        self.prev_seal = GENESIS
        self.sealed = True
        self._load()

    # ---- files --------------------------------------------------------

    def _seg_file(self, n: int) -> str:
        return os.path.join(self.path, f"seg-{n:06d}.log")

    def segments(self) -> list[int]:
        return sorted(int(f[4:10]) for f in os.listdir(self.path) if f.startswith("seg-") and f.endswith(".log"))

    def _append_line(self, line: str) -> Addr:
        with open(self._seg_file(self.seg), "a", encoding="utf-8") as f:
            f.write(line + "\n")
            f.flush()
            os.fsync(f.fileno())
        self.seq += 1
        return Addr(self.seg, self.seq)

    # ---- rebuild -----------------------------------------------------

    def _load(self) -> None:
        for n in self.segments():
            self.seg, self.seq = n, 0
            with open(self._seg_file(n), encoding="utf-8") as f:
                for line in f:
                    self.seq += 1
                    rec = Record.parse(line)
                    addr = Addr(n, self.seq)
                    if rec.op == "S" and rec.action == "seal":
                        self.prev_seal = rec.ctx["chain"]
                        self.sealed = True
                    elif rec.op == "S" and rec.action == "open":
                        self.sealed = False
                    self._index(addr, rec)

    def _index(self, addr: Addr, rec: Record) -> None:
        self.records.append((addr, rec))
        self.by_addr[addr] = rec
        rid = self._retry_id(rec)
        if rec.op == "S" and rec.action == "reject":
            self.retry[(rec.ctx["rec"], rid)] = (addr, rec.ctx["digest"], rec.ctx["reason"])
        elif rid is not None and rec.op != "S":
            self.retry[(rec.ctx["rec"], rid)] = (addr, self._claim_digest(rec), None)
        d = self.vocab.get(rec.key)
        if d and d["kind"] == "correction":
            self.superseded[Addr.parse(rec.ctx["ref"])] = addr

    @staticmethod
    def _retry_id(rec: Record) -> str | None:
        if "ps" in rec.ctx:
            return f"ps:{rec.ctx['ps']}"
        if "rk" in rec.ctx:
            return f"rk:{rec.ctx['rk']}"
        return None

    @staticmethod
    def _claim_digest(rec: Record) -> str:
        return _digest(rec.line().encode())

    # ---- segment lifecycle (§6.2, §6.3) ------------------------------

    def _ensure_open(self, ts: int) -> None:
        if not self.sealed:
            return
        self.seg += 1
        self.seq = 0
        ctx = {"stream": self.stream, "vocab": self.vocab.version, "prev": self.prev_seal}
        rec = Record(ts, "S", "open", f"seg-{self.seg:06d}", ctx)
        self._index(self._append_line(rec.line()), rec)
        self.sealed = False

    def seal(self, ts: int) -> str:
        """Close the current segment. `body` is every byte before the seal line."""
        if self.sealed:
            raise RuntimeError("no open segment")
        with open(self._seg_file(self.seg), "rb") as f:
            body = f.read()
        body_hash = _digest(body)
        chain = _digest((self.prev_seal + body_hash).encode())
        ctx = {"n": self.seq, "sha256": body_hash, "chain": chain}
        rec = Record(ts, "S", "seal", f"seg-{self.seg:06d}", ctx)
        self._index(self._append_line(rec.line()), rec)
        self.prev_seal = chain
        self.sealed = True
        return chain

    def verify(self) -> list[str]:
        """Recompute every sealed segment's hash and the chain (§6.3)."""
        problems, prev = [], GENESIS
        for n in self.segments():
            with open(self._seg_file(n), "rb") as f:
                lines = f.read().splitlines(keepends=True)
            head = Record.parse(lines[0].decode())
            if head.ctx.get("prev") != prev:
                problems.append(f"seg {n}: prev link {head.ctx.get('prev')[:12]} != {prev[:12]}")
            last = Record.parse(lines[-1].decode())
            if not (last.op == "S" and last.action == "seal"):
                continue  # open segment: nothing to verify yet
            body = b"".join(lines[:-1])
            if _digest(body) != last.ctx["sha256"]:
                problems.append(f"seg {n}: body hash mismatch")
            if len(lines) - 1 != last.ctx["n"]:
                problems.append(f"seg {n}: record count mismatch")
            expect = _digest((prev + last.ctx["sha256"]).encode())
            if expect != last.ctx["chain"]:
                problems.append(f"seg {n}: chain mismatch")
            prev = last.ctx["chain"]
        return problems

    # ---- capture (§5, §6.5) -------------------------------------------

    def submit(self, rec_id: str, lines: Iterable[str], received_at: int) -> list[Receipt]:
        """Accept a batch from an authenticated recorder.

        A batch is transport, not a transaction: each record is validated and
        committed on its own (§6.4). Atomic multi-leg claims are single records.
        """
        out = []
        for line in lines:
            try:
                rec = Record.parse(line)
            except (ValueError, json.JSONDecodeError) as e:
                out.append(Receipt("rejected", reason=f"malformed: {e}"))
                continue
            out.append(self._submit_one(rec_id, rec, received_at))
        return out

    def _submit_one(self, rec_id: str, rec: Record, received_at: int) -> Receipt:
        # §5.1: the recorder is whoever authenticated, never what the line says.
        claimed = rec.ctx.get("rec")
        if claimed is not None and claimed != rec_id:
            return Receipt("rejected", reason=f"rec {claimed!r} does not match submitter {rec_id!r}")
        rec = Record(rec.ts, rec.op, rec.action, rec.obj, {**rec.ctx, "rec": rec_id})

        # §6.5: retry identity is (recorder, ps|rk), checked before validation so
        # a retried submission always gets its original receipt back.
        rid = self._retry_id(rec)
        if rid is not None and (rec_id, rid) in self.retry:
            addr, digest, reason = self.retry[(rec_id, rid)]
            if digest != self._claim_digest(rec):
                return Receipt("rejected", reason=f"{rid} already used by a different claim at {addr}")
            if reason is not None:
                return Receipt("rejected", addr=str(addr), reason=reason)
            return Receipt("duplicate", addr=str(addr))

        try:
            self._validate(rec)
        except Rejected as e:
            if rid is None:
                return Receipt("rejected", reason=str(e))
            # §6.5: a rejection with a retry id is journaled, so the producer's
            # sequence stays accounted for and a retry gets the same answer.
            self._ensure_open(received_at)
            rj = Record(received_at, "S", "reject", rec.key,
                        {"rec": rec_id, rid.split(":")[0]: rec.ctx[rid.split(":")[0]],
                         "digest": self._claim_digest(rec), "reason": str(e)})
            addr = self._append_line(rj.line())
            self._index(addr, rj)
            return Receipt("rejected", addr=str(addr), reason=str(e))

        self._ensure_open(received_at)
        addr = self._append_line(rec.line())
        self._index(addr, rec)
        return Receipt("accepted", addr=str(addr))

    # ---- validation (§5) ---------------------------------------------

    def _validate(self, rec: Record) -> None:
        if rec.op == "S":
            raise Rejected("S records are written by the journal, not submitted")
        d = self.vocab.get(rec.key)
        if d is None:
            raise Rejected(f"no definition for {rec.key} in {self.vocab.version}")
        for k in d.get("requires", []):
            if k not in rec.ctx:
                raise Rejected(f"{rec.key} requires ctx.{k}")
        for k, allowed in d.get("enum", {}).items():
            if rec.ctx.get(k) not in allowed:
                raise Rejected(f"ctx.{k} must be one of {allowed}")
        for k in d.get("decimal", []):
            _dec(rec.ctx[k], f"ctx.{k}")
        for k in d.get("qtymaps", []):
            if k in rec.ctx:
                m = rec.ctx[k]
                if not isinstance(m, dict) or not m:
                    raise Rejected(f"ctx.{k} must be a non-empty object of quantity code -> decimal string")
                for code, v in m.items():
                    _dec(v[1:] if isinstance(v, str) and v.startswith("~") else v, f"ctx.{k}.{code}")
        for k in d.get("refs", []):
            for ref in rec.ctx[k]:
                self._resolve(ref)
        if "tf" in rec.ctx and not (isinstance(rec.ctx["tf"], int) and 0 <= rec.ctx["tf"] < 1_000_000):
            raise Rejected("ctx.tf must be an integer 0..999999 (microseconds)")

        actor = rec.ctx.get("actor", rec.ctx["rec"])
        if not self.grants.allows(rec.ctx["rec"], actor):
            raise Rejected(f"{rec.ctx['rec']} may not record claims for {actor}")

        if d["kind"] in ("posting", "correction"):
            if "legs" in rec.ctx:
                self._check_balanced(rec.ctx["legs"])
            if d["kind"] == "correction":
                target = self._resolve(rec.ctx["ref"])
                # An amendment is itself correctable; a reversal is final.
                if target.key not in (d["corrects"], "L.amend"):
                    raise Rejected(f"{rec.key} cannot correct {target.key}")
                ref = Addr.parse(rec.ctx["ref"])
                if ref in self.superseded:
                    raise Rejected(f"{ref} already superseded by {self.superseded[ref]}; correct the head of the chain")

    def _resolve(self, ref: str) -> Record:
        try:
            return self.by_addr[Addr.parse(ref)]
        except (KeyError, ValueError):
            raise Rejected(f"reference {ref!r} is not a committed address")

    @staticmethod
    def _check_balanced(legs) -> None:
        """§8.2: each transaction balances on its own, per (book, unit)."""
        if not isinstance(legs, list) or len(legs) < 2:
            raise Rejected("a posting needs at least two legs")
        sums: dict[tuple[str, str], Decimal] = {}
        for i, leg in enumerate(legs):
            if not isinstance(leg, dict) or not {"a", "q", "u"} <= leg.keys():
                raise Rejected(f"leg {i} needs a, q, u")
            q = _dec(leg["q"], f"leg {i} q")
            if q == 0:
                raise Rejected(f"leg {i} has zero quantity")
            k = (leg.get("book", ""), leg["u"])
            sums[k] = sums.get(k, Decimal(0)) + q
        bad = {f"{b or '-'}/{u}": str(s) for (b, u), s in sums.items() if s != 0}
        if bad:
            raise Rejected(f"unbalanced: {bad}")


# --------------------------------------------------------------------------
# Replay views (§7). Pure functions of the committed records.
# --------------------------------------------------------------------------

def work(j: Journal) -> list[tuple[Addr, Record]]:
    """Committed records minus journal bookkeeping (§7.1)."""
    return [(a, r) for a, r in j.records if r.op != "S"]


def _pair_key(d: dict, r: Record) -> tuple:
    if all(k in r.ctx for k in d.get("pair", [])):
        return tuple(r.ctx[k] for k in d["pair"])
    fb = d.get("pair_fallback")
    if not fb:
        return ("?", r.obj)
    return tuple(r.obj if k == "object" else r.actor() if k == "actor" else r.ctx.get(k) for k in fb)


def intervals(j: Journal, op: str) -> dict:
    """Pair open/close boundaries by pairing key, in occurrence order (§7.3).

    Anomalies are reported, never repaired: a second open on an open key and a
    close with no open are both surfaced for an actor to reconcile.
    """
    open_: dict[tuple, tuple[Addr, Record]] = {}
    done, anomalies = [], []
    rows = sorted(((a, r) for a, r in work(j) if r.op == op), key=lambda x: (x[1].ts, x[1].ctx.get("tf", 0), x[0].seg, x[0].seq))
    for a, r in rows:
        d = j.vocab.get(r.key)
        if not d or d["kind"] != "boundary":
            continue
        k = _pair_key(d, r)
        if d["edge"] == "open":
            if k in open_:
                anomalies.append({"kind": "reopen", "key": k, "at": str(a), "prior": str(open_[k][0])})
            open_[k] = (a, r)
        elif k in open_:
            sa, s = open_.pop(k)
            dur_us = (r.ts - s.ts) * 1_000_000 + r.ctx.get("tf", 0) - s.ctx.get("tf", 0)
            done.append({"key": k, "object": s.obj, "actor": s.actor(), "start": str(sa), "end": str(a),
                         "dur_s": dur_us / 1e6, "reconciled": "reconciled" in r.ctx,
                         "status": r.ctx.get("status"), "parent": s.ctx.get("parent")})
        else:
            anomalies.append({"kind": "close-without-open", "key": k, "at": str(a)})
    return {"closed": done, "open": sorted(str(a) for a, _ in open_.values()), "anomalies": anomalies}


def _q(v: str) -> tuple[Decimal, bool]:
    """A use value: decimal string, '~' prefix marks an estimate."""
    return (Decimal(v[1:]), True) if v.startswith("~") else (Decimal(v), False)


def attribution(j: Journal, conserved: Iterable[str]) -> dict:
    """For each closed run: what its children account for, and what is left (§N5).

    Conserved quantities (energy, cpu time, tokens, bytes, money) add up: a
    parent's use is at least the sum of its children's, and the difference is
    work done at a resolution below what was recorded. Wall time does not add
    up under concurrency, so it is reported as an envelope and a parallelism
    ratio instead.
    """
    conserved = set(conserved)
    closed = {i["key"][0]: i for i in intervals(j, "R")["closed"]}
    use = {}
    for _, r in work(j):
        if r.key == "R.close" and "use" in r.ctx:
            use[r.ctx["run"]] = r.ctx["use"]
    out = {}
    for run, iv in closed.items():
        kids = [k for k, c in closed.items() if c["parent"] == run]
        mine = use.get(run, {})
        row = {"wall_s": iv["dur_s"], "children": sorted(kids), "remainder": {}, "over": {}}
        if kids and iv["dur_s"]:
            row["parallelism"] = round(sum(closed[k]["dur_s"] for k in kids) / iv["dur_s"], 3)
        if "xs" in mine and iv["dur_s"]:
            row["utilisation"] = round(float(_q(mine["xs"])[0]) / iv["dur_s"], 3)
        for code, v in mine.items():
            if code not in conserved:
                continue
            total, est = _q(v)
            parts = [_q(use[k][code]) for k in kids if code in use.get(k, {})]
            rem = total - sum((p[0] for p in parts), Decimal(0))
            est = est or any(p[1] for p in parts)
            (row["over"] if rem < 0 else row["remainder"])[code] = ("~" if est else "") + str(rem)
        out[run] = row
    return out


def tasks(j: Journal) -> dict:
    out: dict[str, dict] = {}
    for a, r in work(j):
        if r.op != "T":
            continue
        t = out.setdefault(r.obj, {"status": None, "fields": {}, "history": []})
        t["history"].append(str(a))
        if r.action == "add":
            t["status"] = "pending"
            t["fields"]["desc"] = r.ctx["desc"]
        elif r.action == "modify":
            t["fields"].update(r.ctx["set"])
        elif r.action == "start":
            t["status"] = "active"
        elif r.action == "done":
            t["status"] = "completed"
        elif r.action == "delete":
            t["status"] = "deleted"
    return out


def _effective_postings(j: Journal) -> list[tuple[Addr, Record]]:
    """A posting counts unless superseded; an amendment counts unless superseded."""
    out = []
    for a, r in work(j):
        if r.key in ("L.post", "L.amend") and a not in j.superseded:
            out.append((a, r))
    return out


def balances(j: Journal) -> dict:
    """Balances per (book, account, unit), each with the addresses it came from."""
    bal: dict[str, dict] = {}
    for a, r in _effective_postings(j):
        for leg in r.ctx["legs"]:
            k = f"{leg.get('book', '-')}|{leg['a']}|{leg['u']}"
            e = bal.setdefault(k, {"q": Decimal(0), "from": []})
            e["q"] += Decimal(leg["q"])
            e["from"].append(str(a))
    return {k: {"q": str(v["q"]), "from": v["from"]} for k, v in sorted(bal.items())}


def provenance(j: Journal, addr: str) -> list[str]:
    """Walk a correction chain back to the original evidence (§8.3)."""
    chain, cur = [addr], j.by_addr[Addr.parse(addr)]
    while "ref" in cur.ctx and (j.vocab.get(cur.key) or {}).get("kind") == "correction":
        chain.append(cur.ctx["ref"])
        cur = j.by_addr[Addr.parse(cur.ctx["ref"])]
    return chain


def capture_gaps(j: Journal) -> dict[str, list[int]]:
    """Missing producer sequence numbers per recorder (§6.7)."""
    seen: dict[str, set[int]] = {}
    for _, r in j.records:
        if (r.op != "S" or r.action == "reject") and isinstance(r.ctx.get("ps"), int):
            seen.setdefault(r.ctx["rec"], set()).add(r.ctx["ps"])
    return {rec: sorted(set(range(1, max(s) + 1)) - s) for rec, s in sorted(seen.items()) if set(range(1, max(s) + 1)) - s}


def timeline(j: Journal) -> list[str]:
    """Occurrence-time view; ties broken by address. Never written back (§6.1)."""
    rows = sorted(work(j), key=lambda x: (x[1].ts, x[1].ctx.get("tf", 0), x[0].seg, x[0].seq))
    return [str(a) for a, _ in rows]


def price(j: Journal, tariff: dict) -> dict[str, str]:
    """Reprice measured usage under a tariff without touching the journal (§9.4).

    tariff: {"version": str, "rates": {unit: decimal-string per unit}}
    Returns cost per run, from every M.measure whose unit has a rate.
    """
    cost: dict[str, Decimal] = {}
    for _, r in work(j):
        if r.key == "M.measure" and r.ctx["u"] in tariff["rates"]:
            cost[r.obj] = cost.get(r.obj, Decimal(0)) + Decimal(r.ctx["q"]) * Decimal(tariff["rates"][r.ctx["u"]])
    return {k: str(v) for k, v in sorted(cost.items())}


def snapshot(j: Journal) -> dict:
    """Every core view, for rebuild-equivalence checks."""
    return {
        "B": intervals(j, "B"),
        "R": intervals(j, "R"),
        "tasks": tasks(j),
        "balances": balances(j),
        "gaps": capture_gaps(j),
        "timeline": timeline(j),
    }
