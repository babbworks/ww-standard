"""Run form: a state-once, glyph-based text encoding of journal lines.

See ../STREAM-NOTATION-v0.1-DRAFT.md. The only normative property is that
decode(encode(lines)) == lines, byte for byte. The encoder is greedy and
non-normative; any encoder that produces decodable output is conforming.

Grammar (one run = one header line followed by rows):

    header  = "@" ts *(SP item)
    item    = "%dt=" int | "%op=" OPglyph | "%obj=" token | "%ps=" int | key "=" literal
    row     = time SP OPglyph SP object *(SP positional) [SP json-object]     ; ordinary run
              (with %ps, a row's ps is the previous row's + 1 unless its json restates it)
            | positional *(SP positional) | "_"                               ; template run (%op,%obj,%dt)
    time    = ("+"|"-") int ["." 1*6digit]   ; whole-second delta, then the record's own sub-second part
    OPglyph = OP glyph | OP "." action       ; e.g. "B(" or "T.track" when no glyph is defined

Header items apply to every row of the run. Rows carry only what differs.
"""

from __future__ import annotations

import json
import re

from stream_journal import Record, canon_ctx

_DEC = json.JSONDecoder()
_QTY = re.compile(r"^(~?[+-]?\d+(?:\.\d+)?)([^\d\s.~+-][^\s]*)$")
_SAFE = re.compile(r'^[^\s"{\[_=][^\s]*$')
_LITERAL_NONSTR = re.compile(r"^(-?\d+(\.\d+)?([eE][+-]?\d+)?|true|false|null)$")


class Notation:
    def __init__(self, path: str):
        with open(path, encoding="utf-8") as f:
            doc = json.load(f)
        self.version = doc["notation"]
        self.glyph = doc["glyphs"]
        self.unglyph = {}
        for key, g in self.glyph.items():
            op = key.split(".")[0]
            self.unglyph[op + g] = key
        self.pos = {k: [tuple(x) for x in v] for k, v in doc["positional"].items()}
        self.quantities = doc["quantities"]

    # ---- op tokens --------------------------------------------------

    def op_token(self, rec: Record) -> str:
        g = self.glyph.get(rec.key)
        return rec.op + g if g else f"{rec.op}.{rec.action}"

    def op_parse(self, tok: str) -> tuple[str, str]:
        if tok in self.unglyph:
            op, action = self.unglyph[tok].split(".")
            return op, action
        op, action = tok.split(".", 1)
        return op, action


# ---- token helpers --------------------------------------------------

def _tokens(s: str) -> list:
    """Split on spaces; a token starting with '"' or '{' is one JSON value."""
    out, i, n = [], 0, len(s)
    while i < n:
        if s[i] == " ":
            i += 1
            continue
        if s[i] in '"{[':
            v, j = _DEC.raw_decode(s, i)
            out.append(("json", v))
            i = j
        else:
            j = s.find(" ", i)
            j = n if j < 0 else j
            out.append(("bare", s[i:j]))
            i = j
    return out


def _safe(v) -> bool:
    return isinstance(v, str) and bool(_SAFE.match(v))


def _qty_ok(q, u) -> bool:
    return isinstance(q, str) and isinstance(u, str) and bool(_QTY.match(q + u)) and _QTY.match(q + u).groups() == (q, u)


def _literal(v) -> str:
    if isinstance(v, str):
        if _safe(v) and not _LITERAL_NONSTR.match(v) and not v.startswith("%"):
            return v
        return json.dumps(v, ensure_ascii=False)
    return json.dumps(v, separators=(",", ":"), ensure_ascii=False)


def _item(k: str, v) -> str:
    lit = _literal(v)
    return f"{k}= {lit}" if lit[0] in '"{[' else f"{k}={lit}"


def _unliteral(tok) -> object:
    kind, v = tok
    if kind == "json":
        return v
    if _LITERAL_NONSTR.match(v):
        return json.loads(v)
    return v


def _time_token(dts: int, rec: Record) -> str:
    t = ("+" if dts >= 0 else "-") + str(abs(dts))
    if "tf" in rec.ctx:
        frac = f"{rec.ctx['tf']:06d}".rstrip("0") or "0"
        t += "." + frac
    return t


# ---- positional payload ----------------------------------------------

def _encode_positional(spec, ctx: dict) -> tuple[list[str], set]:
    """Longest prefix of the spec that can be written as tokens."""
    toks, used = [], set()
    for key, typ in spec:
        if typ == "qty":
            if not ("q" in ctx and "u" in ctx and _qty_ok(ctx["q"], ctx["u"])):
                break
            toks.append(ctx["q"] + ctx["u"]); used |= {"q", "u"}
        elif key not in ctx:
            break
        elif typ == "str":
            if not _safe(ctx[key]):
                break
            toks.append(ctx[key]); used.add(key)
        elif typ == "int":
            if not (isinstance(ctx[key], int) and not isinstance(ctx[key], bool)):
                break
            toks.append(str(ctx[key])); used.add(key)
        elif typ == "text":
            if not isinstance(ctx[key], str):
                break
            toks.append(json.dumps(ctx[key], ensure_ascii=False)); used.add(key)
        elif typ == "legs":
            legs = ctx[key]
            if not (isinstance(legs, list) and all(isinstance(l, dict) and set(l) == {"a", "q", "u"} and _safe(l["a"])
                                                    and "=" not in l["a"] and _qty_ok(l["q"], l["u"]) for l in legs)):
                break
            toks += [f"{l['a']}={l['q']}{l['u']}" for l in legs]; used.add(key)
        elif typ == "qtymap":
            m = ctx[key]
            if not (isinstance(m, dict) and m and all(_qty_ok(v, k) for k, v in m.items())):
                break
            toks += [v + k for k, v in m.items()]; used.add(key)
        else:
            raise ValueError(typ)
    return toks, used


def _decode_positional(spec, toks: list) -> dict:
    ctx, i = {}, 0
    for key, typ in spec:
        if i >= len(toks) or toks[i][0] == "json" and typ != "text":
            break
        if typ == "qty":
            ctx["q"], ctx["u"] = _QTY.match(toks[i][1]).groups(); i += 1
        elif typ == "str":
            ctx[key] = toks[i][1]; i += 1
        elif typ == "int":
            ctx[key] = int(toks[i][1]); i += 1
        elif typ == "text":
            if toks[i][0] != "json" or not isinstance(toks[i][1], str):
                break
            ctx[key] = toks[i][1]; i += 1
        elif typ == "legs":
            legs = []
            while i < len(toks) and toks[i][0] == "bare":
                a, qty = toks[i][1].split("=", 1)
                q, u = _QTY.match(qty).groups()
                legs.append({"a": a, "q": q, "u": u}); i += 1
            ctx[key] = legs
        elif typ == "qtymap":
            m = {}
            while i < len(toks) and toks[i][0] == "bare":
                q, u = _QTY.match(toks[i][1]).groups()
                m[u] = q; i += 1
            ctx[key] = m
    return ctx, toks[i:]


# ---- encoder ----------------------------------------------------------

_MIN_TEMPLATE = 3


def encode(lines: list[str], nt: Notation) -> str:
    recs = [Record.parse(l) for l in lines]
    out: list[str] = []
    i = 0
    while i < len(recs):
        j = _template_extent(recs, i, nt)
        if j - i >= _MIN_TEMPLATE:
            out += _emit_template(recs[i:j], nt)
        else:
            j = _emit_run(recs, i, nt, out)
        i = j
    return "\n".join(out) + "\n"


def _row_parts(r: Record, nt: Notation):
    spec = nt.pos.get(r.key, [])
    toks, used = _encode_positional(spec, r.ctx)
    return toks, used


def _shareable(r: Record, used: set) -> dict:
    return {k: v for k, v in r.ctx.items() if k not in used and k not in ("ps", "tf")}


def _template_extent(recs, i, nt) -> int:
    first = recs[i]
    if "tf" in first.ctx or not isinstance(first.ctx.get("ps"), int):
        return i
    toks, used = _row_parts(first, nt)
    shared = _shareable(first, used)
    j, dt = i + 1, None
    while j < len(recs):
        r = recs[j]
        t, u = _row_parts(r, nt)
        if (r.key, r.obj) != (first.key, first.obj) or "tf" in r.ctx or u != used or _shareable(r, u) != shared:
            break
        if r.ctx.get("ps") != first.ctx["ps"] + (j - i):
            break
        d = r.ts - recs[j - 1].ts
        if d <= 0 or (dt is not None and d % dt):
            break
        dt = d if dt is None else dt
        j += 1
    # Cadence must be exact multiples of one step (gaps become "_").
    if dt is None:
        return i
    return j


def _emit_template(rs, nt) -> list[str]:
    first = rs[0]
    _, used = _row_parts(first, nt)
    dt = min(rs[k + 1].ts - rs[k].ts for k in range(len(rs) - 1))
    head = [f"@{first.ts}", f"%dt={dt}", f"%op={nt.op_token(first)}", f"%obj={first.obj}", f"%ps={first.ctx['ps']}"]
    head += [_item(k, v) for k, v in sorted(_shareable(first, used).items())]
    out = [" ".join(head)]
    prev = first.ts - dt
    for r in rs:
        while r.ts - prev > dt:
            out.append("_"); prev += dt
        out.append(" ".join(_row_parts(r, nt)[0]))
        prev = r.ts
    return out


def _emit_run(recs, i, nt, out) -> int:
    first = recs[i]
    _, used = _row_parts(first, nt)
    shared = _shareable(first, used)
    ps0 = first.ctx.get("ps") if isinstance(first.ctx.get("ps"), int) else None
    head = [f"@{first.ts}"] + ([f"%ps={ps0}"] if ps0 is not None else [])
    head += [_item(k, v) for k, v in sorted(shared.items())]
    out.append(" ".join(head))
    prev, j, expect = first.ts, i, ps0
    while j < len(recs):
        r = recs[j]
        toks, used = _row_parts(r, nt)
        if j > i:
            if any(r.ctx.get(k, object()) != v for k, v in shared.items()) or any(k in used for k in shared):
                break
            if ps0 is not None and not isinstance(r.ctx.get("ps"), int):
                break
            if _template_extent(recs, j, nt) - j >= _MIN_TEMPLATE:
                break
        rest = {k: v for k, v in r.ctx.items() if k not in used and k not in shared and k != "tf"
                and not (k == "ps" and ps0 is not None and v == expect)}
        if ps0 is not None:
            expect = r.ctx["ps"] + 1
        row = [_time_token(r.ts - prev, r), nt.op_token(r), r.obj] + toks
        if rest:
            row.append(canon_ctx(rest))
        out.append(" ".join(row))
        prev = r.ts
        j += 1
    return j


# ---- decoder ----------------------------------------------------------

def decode(text: str, nt: Notation) -> list[str]:
    out: list[str] = []
    head = None
    for raw in text.splitlines():
        if not raw:
            continue
        if raw.startswith("@"):
            head = _parse_header(raw)
            continue
        if "dt" in head["dir"]:
            head["t"] += head["dir"]["dt"]
            if raw == "_":
                continue
            op, action = nt.op_parse(head["dir"]["op"])
            obj, ts, toks = head["dir"]["obj"], head["t"], _tokens(raw)
            tf = None
        else:
            tm, opg, obj, rest = (raw.split(" ", 3) + [""])[:4]
            m = re.match(r"^([+-])(\d+)(?:\.(\d{1,6}))?$", tm)
            head["t"] += int(m.group(2)) * (1 if m.group(1) == "+" else -1)
            ts, tf = head["t"], (int(m.group(3).ljust(6, "0")) if m.group(3) is not None else None)
            op, action = nt.op_parse(opg)
            toks = _tokens(rest)
        ctx, left = _decode_positional(nt.pos.get(f"{op}.{action}", []), toks)
        for kind, v in left:
            if kind != "json" or not isinstance(v, dict):
                raise ValueError(f"unexpected token {v!r} in {raw!r}")
            ctx.update(v)
        ctx.update(head["ctx"])
        if tf is not None:
            ctx["tf"] = tf
        if head["ps"] is not None:
            if "ps" not in ctx:            # a row may restate ps; the count continues from it
                ctx["ps"] = head["ps"]
            head["ps"] = ctx["ps"] + 1
        out.append(Record(ts, op, action, obj, ctx).line())
    return out


def _parse_header(raw: str) -> dict:
    toks = _tokens(raw)
    base = int(toks[0][1][1:])
    h = {"dir": {}, "ctx": {}, "ps": None}
    pending_key = None
    for kind, v in toks[1:]:
        if pending_key is not None:          # key="json literal" split by the tokenizer
            h["ctx"][pending_key] = v
            pending_key = None
            continue
        k, _, val = v.partition("=")
        if k == "%dt":
            h["dir"]["dt"] = int(val)
        elif k == "%op":
            h["dir"]["op"] = val
        elif k == "%obj":
            h["dir"]["obj"] = val
        elif k == "%ps":
            h["ps"] = int(val)
        elif val == "" and v.endswith("="):
            pending_key = k
        else:
            h["ctx"][k] = _unliteral(("bare", val))
    h["t"] = base - h["dir"]["dt"] if "dt" in h["dir"] else base
    return h
