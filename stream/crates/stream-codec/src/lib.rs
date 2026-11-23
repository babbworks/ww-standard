//! The run form: say shared context once, then only what changes.
//!
//! ```text
//! @1752300096 %ps=1 iv=8 rec=meter:pdu-4 scope=host:build-01 u=J
//! +0 MEASURE* pdu-4.out3 {"q":"38"}
//! +8 MEASURE* pdu-4.out3 {"q":"41"}
//! ```
//!
//! * A header `@<time>` starts a run. `key=value` items are shared by every
//!   row in it. `%ps=<n>` numbers rows n, n+1, … unless a row restates `ps`.
//! * A row is `<±seconds>[.fraction] <type> <object>[ <json>]`: whole-second
//!   delta from the previous row, then the entry's own sub-second part.
//!   The JSON holds only payload keys the header doesn't supply.
//!
//! The one rule that matters: `decode(encode(lines)) == lines`, byte for byte.
//! The encoder is greedy and not normative; any encoder that decodes is valid.

#![no_std]
extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use stream_kernel::json::{object_to_string, parse_object, parse_value_prefix, write_value, Object, Value};
use stream_kernel::{Entry, KernelError, Time};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodecError(pub String);

fn err<T>(m: impl ToString) -> Result<T, CodecError> {
    Err(CodecError(m.to_string()))
}

impl From<KernelError> for CodecError {
    fn from(e: KernelError) -> Self {
        CodecError(e.to_string())
    }
}

fn is_reserved_bare(v: &str) -> bool {
    v.is_empty()
        || v.starts_with(['"', '{', '[', '%'])
        || v.contains(' ')
        || v.chars().any(char::is_control)
        || v == "true"
        || v == "false"
        || v == "null"
        || v.parse::<i64>().is_ok()
        || (v.starts_with('-') || v.starts_with(|c: char| c.is_ascii_digit())) && v.trim_start_matches('-').chars().all(|c| c.is_ascii_digit())
}

fn item(k: &str, v: &Value, out: &mut String) {
    out.push(' ');
    out.push_str(k);
    out.push('=');
    match v {
        Value::Str(s) if !is_reserved_bare(s) => out.push_str(s),
        Value::Int(_) | Value::Bool(_) | Value::Null => write_value(v, out),
        _ => {
            out.push(' ');
            write_value(v, out);
        }
    }
}

fn key_ok(k: &str) -> bool {
    !k.is_empty() && !k.starts_with('%') && !k.contains(['=', ' ']) && !k.chars().any(char::is_control)
}

/// Encode canonical lines as run form. Lines must already be canonical.
pub fn encode(lines: &[String]) -> Result<String, CodecError> {
    let entries: Vec<Entry> = lines.iter().map(|l| Entry::parse(l)).collect::<Result<_, _>>()?;
    let mut out = String::new();
    let mut i = 0;
    while i < entries.len() {
        let first = &entries[i];
        let ps0 = first.get("ps").and_then(Value::as_int);
        // Shared context: what this row has in common with the next one.
        let next = entries.get(i + 1);
        let shared: Object = first
            .payload
            .iter()
            .filter(|(k, v)| *k != "ps" && key_ok(k) && next.is_none_or(|n| n.payload.get(*k) == Some(*v)))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        out.push('@');
        out.push_str(&first.time.secs.to_string());
        if let Some(p) = ps0 {
            out.push_str(&alloc::format!(" %ps={p}"));
        }
        for (k, v) in &shared {
            item(k, v, &mut out);
        }
        out.push('\n');
        let mut prev = first.time.secs;
        let mut expect = ps0;
        let mut j = i;
        while j < entries.len() {
            let e = &entries[j];
            if j > i {
                let has_shared = shared.iter().all(|(k, v)| e.payload.get(k) == Some(v));
                let ps_ok = ps0.is_none() || e.get("ps").and_then(Value::as_int).is_some();
                if !has_shared || !ps_ok {
                    break;
                }
            }
            let delta = e.time.secs as i128 - prev as i128;
            out.push_str(&alloc::format!("{}{}", if delta < 0 { "-" } else { "+" }, delta.unsigned_abs()));
            if e.time.micros != 0 {
                let f = alloc::format!("{:06}", e.time.micros);
                out.push('.');
                out.push_str(f.trim_end_matches('0'));
            }
            out.push(' ');
            out.push_str(&e.type_token());
            out.push(' ');
            out.push_str(&e.object);
            let mut rest: Object = e.payload.iter().filter(|(k, _)| !shared.contains_key(*k)).map(|(k, v)| (k.clone(), v.clone())).collect();
            if let (Some(want), Some(got)) = (expect, e.get("ps").and_then(Value::as_int)) {
                if got == want {
                    rest.remove("ps");
                }
                expect = Some(got + 1);
            }
            if !rest.is_empty() {
                out.push(' ');
                out.push_str(&object_to_string(&rest));
            }
            out.push('\n');
            prev = e.time.secs;
            j += 1;
        }
        i = j;
    }
    Ok(out)
}

/// Decode run form back to canonical lines.
pub fn decode(text: &str) -> Result<Vec<String>, CodecError> {
    let mut out = Vec::new();
    let mut shared = Object::new();
    let mut ps: Option<i64> = None;
    let mut prev: Option<u64> = None;
    for (n, line) in text.lines().enumerate() {
        if line.is_empty() {
            continue;
        }
        if let Some(h) = line.strip_prefix('@') {
            let (shr, p, t) = header(h).map_err(|e| CodecError(alloc::format!("line {}: {}", n + 1, e.0)))?;
            shared = shr;
            ps = p;
            prev = Some(t);
            continue;
        }
        let base = prev.ok_or_else(|| CodecError(alloc::format!("line {}: row before any header", n + 1)))?;
        let mut parts = line.splitn(4, ' ');
        let (tm, ty, obj) = match (parts.next(), parts.next(), parts.next()) {
            (Some(a), Some(b), Some(c)) => (a, b, c),
            _ => return err(alloc::format!("line {}: a row needs time, type and object", n + 1)),
        };
        let rest = parts.next();
        let (sign, body) = match tm.as_bytes().first() {
            Some(b'+') => (1i128, &tm[1..]),
            Some(b'-') => (-1i128, &tm[1..]),
            _ => return err(alloc::format!("line {}: bad time delta", n + 1)),
        };
        let (whole, frac) = body.split_once('.').map(|(a, b)| (a, Some(b))).unwrap_or((body, None));
        let d: i128 = whole.parse().map_err(|_| CodecError(alloc::format!("line {}: bad time delta", n + 1)))?;
        let secs = (base as i128 + sign * d) as u64;
        let time_txt = match frac {
            Some(f) => alloc::format!("{secs}.{f}"),
            None => secs.to_string(),
        };
        Time::parse(&time_txt)?;
        let mut payload = shared.clone();
        if let Some(r) = rest {
            for (k, v) in parse_object(r).map_err(|e| CodecError(alloc::format!("line {}: {}", n + 1, e.0)))? {
                payload.insert(k, v);
            }
        }
        if let Some(p) = ps {
            let this = match payload.get("ps").and_then(Value::as_int) {
                Some(v) => v,
                None => {
                    payload.insert("ps".into(), Value::Int(p));
                    p
                }
            };
            ps = Some(this + 1);
        }
        let line = alloc::format!("{time_txt} {ty} {obj} {}", object_to_string(&payload));
        Entry::parse(&line).map_err(|e| CodecError(alloc::format!("line {}: {e}", n + 1)))?;
        out.push(line);
        prev = Some(secs);
    }
    Ok(out)
}

fn header(h: &str) -> Result<(Object, Option<i64>, u64), CodecError> {
    let (t, mut rest) = h.split_once(' ').unwrap_or((h, ""));
    let time: u64 = t.parse().map_err(|_| CodecError("bad header time".into()))?;
    let mut shared = Object::new();
    let mut ps = None;
    while !rest.is_empty() {
        let (k, after) = rest.split_once('=').ok_or_else(|| CodecError("header item needs =".into()))?;
        if k == "%ps" {
            let (v, r) = after.split_once(' ').unwrap_or((after, ""));
            ps = Some(v.parse().map_err(|_| CodecError("bad %ps".into()))?);
            rest = r;
            continue;
        }
        if let Some(json) = after.strip_prefix(' ') {
            let (v, used) = parse_value_prefix(json).map_err(|e| CodecError(e.0.into()))?;
            shared.insert(k.to_string(), v);
            rest = json[used..].strip_prefix(' ').unwrap_or(&json[used..]);
        } else {
            let (v, r) = after.split_once(' ').unwrap_or((after, ""));
            let val = match v {
                "true" => Value::Bool(true),
                "false" => Value::Bool(false),
                "null" => Value::Null,
                _ => match v.parse::<i64>() {
                    Ok(i) if i.to_string() == v => Value::Int(i),
                    _ => Value::str(v),
                },
            };
            shared.insert(k.to_string(), val);
            rest = r;
        }
    }
    Ok((shared, ps, time))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;

    fn rt(lines: &[String]) -> String {
        let enc = encode(lines).unwrap();
        assert_eq!(&decode(&enc).unwrap(), lines, "\n{enc}");
        enc
    }

    #[test]
    fn meter_run_is_small_and_exact() {
        let lines: Vec<String> = (0..1000u64)
            .map(|i| format!(r#"{} MEASURE* pdu-4.out3 {{"iv":8,"ps":{},"q":"{}","rec":"meter:pdu-4","scope":"host:build-01","u":"J"}}"#, 1752300096 + 8 * i, i + 1, (i * 37) % 64))
            .collect();
        let enc = rt(&lines);
        let canon: usize = lines.iter().map(|l| l.len() + 1).sum();
        // Shared context and time deltas alone: about 3.2x. Positional payloads
        // and template rows (not yet ported from the trial) take it to ~28x.
        assert!(enc.len() * 3 < canon, "run form {} vs canonical {canon}", enc.len());
        assert!(enc.starts_with("@1752300096 %ps=1 iv=8 rec=meter:pdu-4 scope=host:build-01 u=J\n+0 MEASURE* pdu-4.out3 {\"q\":\"0\"}\n"));
    }

    #[test]
    fn awkward_values_survive() {
        let lines: Vec<String> = [
            r#"1752300000 NOTE x {"label":"123","rec":"a","text":"has spaces and = signs"}"#,
            r#"1752300001.25 B:ATTEND( lathe-01 {"ps":7,"rec":"human:op-03","run":"p1"}"#,
            r#"1752299990 B:ATTEND) lathe-01 {"ps":9,"rec":"human:op-03","run":"p1"}"#,
            r#"1752300002 PAY r1 {"legs":[{"a":"inv:raw","q":"-1","u":"kg"},{"a":"x","q":"1","u":"kg"}],"rec":"%odd","tags":["a","b"],"z":null,"t":true}"#,
        ]
        .iter()
        .map(|s| {
            // Re-canonicalise the hand-written last line's key order.
            let e = stream_kernel::Entry::new(Time::secs(0), stream_kernel::Type::from_byte(0), "x", parse_object(s.splitn(4, ' ').nth(3).unwrap()).unwrap());
            let head: Vec<&str> = s.splitn(4, ' ').take(3).collect();
            format!("{} {} {} {}", head[0], head[1], head[2], object_to_string(&e.payload))
        })
        .collect();
        rt(&lines);
    }
}
