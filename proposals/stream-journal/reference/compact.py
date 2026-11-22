"""Compact encodings for STREAM records (§10 of the draft).

The only rule that matters: decode(encode(records)) must reproduce the
canonical journal lines byte for byte. Size claims are made over the whole
batch: context dictionary + block header + payload + exceptions.

PB1 — periodic measurement block
--------------------------------
For M.measure records from one recorder with identical shared context and a
fixed cadence. Big-endian.

    off size field
      0   1  magic 'P'
      1   1  version (1)
      2   2  ctx_id      index into the batch's context dictionary
      4   4  t0          unix seconds of slot 0
      8   2  cadence     seconds between slots
     10   2  n           number of slots
     12   1  bits        bits per packed value (1..32)
     13   1  exp         signed decimal exponent: q = (base + raw) * 10^exp
     14   4  base        signed
     18   4  ps0         producer sequence of the first present sample
     22   1  nexc        number of exceptions
     23   …  exceptions  idx:u16 kind:u8 [value:i32 if kind=1]
          …  payload     n * bits, MSB first, zero-padded to a byte

Exception kinds: 0 = slot missing (no record, no ps consumed),
                 1 = value outside the packed range (raw value given).
"""

from __future__ import annotations

import json
import struct
from decimal import Decimal

from stream_journal import Record, canon_ctx

HDR = struct.Struct(">cBHIHHBbiIB")
assert HDR.size == 23


class NotEncodable(ValueError):
    pass


def context_entry(rec: Record) -> bytes:
    """The shared part of a record: everything except ts, ps and q."""
    shared = {k: v for k, v in rec.ctx.items() if k not in ("ps", "q")}
    return canon_ctx({"op": rec.op, "action": rec.action, "obj": rec.obj, "ctx": shared}).encode()


def _scaled(q: str, exp: int) -> int:
    v = Decimal(q).scaleb(-exp)
    if v != v.to_integral_value():
        raise NotEncodable(f"{q} not representable at exponent {exp}")
    return int(v)


def _fmt(raw: int, exp: int) -> str:
    # Canonical decimal string: no exponent notation, no trailing zeros beyond exp.
    d = Decimal(raw).scaleb(exp)
    return format(d, "f")


def encode_pb1(records: list[Record], ctx_id: int, cadence: int, bits: int, base: int = 0, exp: int = 0) -> bytes:
    if not records:
        raise NotEncodable("empty")
    shared = context_entry(records[0])
    t0, ps0 = records[0].ts, records[0].ctx["ps"]
    slots: dict[int, int] = {}
    for i, r in enumerate(records):
        if context_entry(r) != shared:
            raise NotEncodable("shared context differs")
        if "tf" in r.ctx:
            raise NotEncodable("sub-second timing needs a different block")
        off = r.ts - t0
        if off < 0 or off % cadence:
            raise NotEncodable(f"record {i} is off cadence")
        if r.ctx["ps"] != ps0 + i:
            raise NotEncodable("producer sequence is not contiguous over present samples")
        if _fmt(_scaled(r.ctx["q"], exp), exp) != r.ctx["q"]:
            raise NotEncodable(f"{r.ctx['q']!r} is not in canonical form for exponent {exp}")
        slots[off // cadence] = _scaled(r.ctx["q"], exp)
    n = max(slots) + 1
    hi = (1 << bits) - 1
    exc, raws = [], []
    for s in range(n):
        if s not in slots:
            exc.append(struct.pack(">HB", s, 0)); raws.append(0)
            continue
        raw = slots[s] - base
        if 0 <= raw <= hi:
            raws.append(raw)
        else:
            exc.append(struct.pack(">HBi", s, 1, slots[s])); raws.append(0)
    if len(exc) > 255:
        raise NotEncodable("too many exceptions; split the block")
    acc = nbits = 0
    payload = bytearray()
    for raw in raws:
        acc = (acc << bits) | raw
        nbits += bits
        while nbits >= 8:
            nbits -= 8
            payload.append((acc >> nbits) & 0xFF)
    if nbits:
        payload.append((acc << (8 - nbits)) & 0xFF)
    hdr = HDR.pack(b"P", 1, ctx_id, t0, cadence, n, bits, exp, base, ps0, len(exc))
    return hdr + b"".join(exc) + bytes(payload)


def decode_pb1(block: bytes, dictionary: list[bytes]) -> list[Record]:
    magic, ver, ctx_id, t0, cadence, n, bits, exp, base, ps0, nexc = HDR.unpack_from(block, 0)
    if magic != b"P" or ver != 1:
        raise ValueError("not a PB1 block")
    entry = json.loads(dictionary[ctx_id])
    off = HDR.size
    missing, override = set(), {}
    for _ in range(nexc):
        idx, kind = struct.unpack_from(">HB", block, off); off += 3
        if kind == 0:
            missing.add(idx)
        else:
            (override[idx],) = struct.unpack_from(">i", block, off); off += 4
    payload = block[off:]
    out, ps = [], ps0
    for s in range(n):
        bitpos = s * bits
        raw = 0
        for b in range(bits):
            byte = payload[(bitpos + b) // 8]
            raw = (raw << 1) | ((byte >> (7 - (bitpos + b) % 8)) & 1)
        if s in missing:
            continue
        val = override.get(s, base + raw)
        ctx = {**entry["ctx"], "ps": ps, "q": _fmt(val, exp)}
        out.append(Record(t0 + s * cadence, entry["op"], entry["action"], entry["obj"], ctx))
        ps += 1
    return out
