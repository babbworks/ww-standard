//! STREAM in the browser.
//!
//! The same log rules as the CLI ([`stream_log::Log`]) over a [`MemStore`],
//! behind one entry point: a JSON request in, a JSON answer out. The page
//! keeps the log's bytes (segments and packs) in browser storage and hands
//! them back with `open` on the next visit.
//!
//! Requests are JSON objects with an `op`. Times are `now_us`, integer
//! microseconds since the Unix epoch. Every answer is a JSON object; a
//! failure has `error`.
//!
//! | op | fields | answer |
//! |---|---|---|
//! | `init` | `log`, `root`, `clock`, `tz`, `now_us` | `log` |
//! | `open` | `segments` (text), `blobs` (hash → text) | `log` |
//! | `adopt` | `as`, `pack` (a bundled pack name), `now_us` | a receipt |
//! | `submit` | `as`, `line`, `now_us` | a receipt |
//! | `check` | `as`, `line` | `ok`, `reason` |
//! | `seal` | `now_us` | `addr` |
//! | `state` | | `segments`, `blobs` |
//! | `verify` | | `problems` |
//! | `now` | | the Now view |
//!
//! Over WebAssembly the entry point is `stream_call`, with `stream_alloc`,
//! `stream_free` and `stream_out_len` for passing bytes. Natively it is
//! [`Session::handle`].

use std::cell::RefCell;
use std::collections::BTreeMap;

use stream_books::{spans, Anomaly, Span};
use stream_kernel::entry::decimal_str;
use stream_kernel::json::{object_to_string, parse_object, Object};
use stream_kernel::{Decimal, Entry, Phase, Time, Value, Verb};
use stream_log::{Genesis, Log, MemStore, Receipt};

/// The packs this build carries, by name.
pub const PACKS: &[(&str, &str)] = &[
    ("si-units@1", include_str!("../../../packs/si-units@1.pack")),
    ("agent-ops@1", include_str!("../../../packs/agent-ops@1.pack")),
];

type Res = Result<Object, String>;

#[derive(Default)]
pub struct Session {
    log: Option<Log>,
}

fn obj<const N: usize>(kv: [(&str, Value); N]) -> Object {
    kv.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}
fn st(s: impl Into<String>) -> Value {
    Value::Str(s.into())
}
fn field<'a>(r: &'a Object, k: &str) -> Result<&'a str, String> {
    r.get(k).and_then(Value::as_str).ok_or_else(|| format!("{k} is required"))
}
fn time(r: &Object) -> Result<Time, String> {
    let us = r.get("now_us").and_then(Value::as_int).filter(|u| *u >= 0).ok_or("now_us is required")?;
    Ok(Time { secs: (us / 1_000_000) as u64, micros: (us % 1_000_000) as u32 })
}
fn us(t: Time) -> Value {
    Value::Int(t.as_micros() as i64)
}
fn qty(d: Decimal, est: bool) -> Value {
    Value::Str(format!("{}{}", if est { "~" } else { "" }, d.normalized()))
}

impl Session {
    pub fn new() -> Session {
        Session::default()
    }

    /// One request, one answer.
    pub fn handle(&mut self, req: &str) -> String {
        let out = parse_object(req).map_err(|e| format!("request is not JSON: {}", e.0)).and_then(|r| self.dispatch(&r));
        object_to_string(&out.unwrap_or_else(|e| obj([("error", Value::Str(e))])))
    }

    fn log(&mut self) -> Result<&mut Log, String> {
        self.log.as_mut().ok_or_else(|| "no log is open".to_string())
    }

    fn dispatch(&mut self, r: &Object) -> Res {
        match field(r, "op")? {
            "init" => {
                let g = Genesis {
                    log: field(r, "log")?,
                    roots: &[field(r, "root")?],
                    clock: field(r, "clock")?,
                    tz: field(r, "tz")?,
                    time: time(r)?,
                };
                let log = Log::init_store(Box::new(MemStore::default()), g).map_err(|e| e.to_string())?;
                self.log = Some(log);
                Ok(obj([("log", st(self.log()?.id()))]))
            }
            "open" => {
                let mut m = MemStore::default();
                for s in r.get("segments").and_then(Value::as_arr).ok_or("segments is required")? {
                    m.segments.push(s.as_str().ok_or("a segment is not text")?.as_bytes().to_vec());
                }
                for (h, b) in r.get("blobs").and_then(Value::as_obj).into_iter().flatten() {
                    m.blobs.insert(h.clone(), b.as_str().ok_or("a blob is not text")?.as_bytes().to_vec());
                }
                let log = Log::open_store(Box::new(m)).map_err(|e| e.to_string())?;
                self.log = Some(log);
                Ok(obj([("log", st(self.log()?.id()))]))
            }
            "adopt" => {
                let name = field(r, "pack")?;
                let text = PACKS.iter().find(|(n, _)| *n == name).ok_or_else(|| format!("no bundled pack {name}"))?.1;
                let (who, now) = (field(r, "as")?.to_string(), time(r)?);
                let log = self.log()?;
                let rc = log.adopt(&who, text.as_bytes(), Object::new(), now).map_err(|e| e.to_string())?;
                Ok(receipt(log, &rc))
            }
            "submit" => {
                let (who, line, now) = (field(r, "as")?.to_string(), field(r, "line")?.to_string(), time(r)?);
                let log = self.log()?;
                let rc = log.submit_line(&who, &line, now).map_err(|e| e.to_string())?;
                Ok(receipt(log, &rc))
            }
            "check" => {
                let who = field(r, "as")?.to_string();
                let e = Entry::parse(field(r, "line")?);
                let log = self.log()?;
                let res = e.map_err(|e| e.to_string()).and_then(|e| log.check_entry(&who, &e));
                Ok(match res {
                    Ok(()) => obj([("ok", Value::Bool(true))]),
                    Err(reason) => obj([("ok", Value::Bool(false)), ("reason", st(reason))]),
                })
            }
            "seal" => {
                let now = time(r)?;
                let a = self.log()?.seal(now).map_err(|e| e.to_string())?;
                Ok(obj([("addr", st(a.to_string()))]))
            }
            "state" => state(self.log()?),
            "verify" => {
                let log = self.log()?;
                let mut p: Vec<Value> = log.verify().into_iter().map(Value::Str).collect();
                p.extend(stream_books::verify_closes(log).into_iter().map(|a| st(format!("close at {a} no longer matches"))));
                Ok(obj([("problems", Value::Arr(p))]))
            }
            "now" => Ok(now_view(self.log()?)),
            op => Err(format!("unknown op {op}")),
        }
    }
}

fn receipt(log: &Log, r: &Receipt) -> Object {
    let (status, reason) = match r {
        Receipt::Accepted(_) => ("accepted", None),
        Receipt::Duplicate(_) => ("duplicate", None),
        Receipt::Rejected { reason, .. } => ("rejected", Some(reason.clone())),
    };
    let mut o = obj([("status", st(status))]);
    if let Some(a) = r.addr() {
        o.insert("addr".into(), st(a.to_string()));
        if let Some(e) = log.get(a) {
            o.insert("line".into(), st(e.to_line()));
        }
    }
    if let Some(x) = reason {
        o.insert("reason".into(), st(x));
    }
    o
}

/// The log's bytes, to keep between visits.
fn state(log: &Log) -> Res {
    let store = log.store();
    let mut segs = vec![];
    let mut n = 1;
    while let Some(b) = store.segment(n).map_err(|e| e.to_string())? {
        segs.push(st(String::from_utf8(b).map_err(|_| "segment is not UTF-8")?));
        n += 1;
    }
    let mut blobs = Object::new();
    for (_, e) in log.entries() {
        if let Some(h) = e.get_str("pack").and_then(|p| p.strip_prefix("sha256:")) {
            if let Some(b) = store.blob(h).map_err(|e| e.to_string())? {
                blobs.insert(h.to_string(), st(String::from_utf8(b).map_err(|_| "pack is not UTF-8")?));
            }
        }
    }
    Ok(obj([("segments", Value::Arr(segs)), ("blobs", Value::Obj(blobs))]))
}

fn uses_obj(m: &BTreeMap<String, (Decimal, bool)>) -> Value {
    Value::Obj(m.iter().map(|(k, (d, e))| (k.clone(), qty(*d, *e))).collect())
}

fn sum_into(m: &mut BTreeMap<String, (Decimal, bool)>, unit: &str, d: Decimal, est: bool) {
    let x = m.entry(unit.to_string()).or_insert((Decimal::ZERO, false));
    if let Some(s) = x.0.checked_add(d) {
        x.0 = s;
    }
    x.1 |= est;
}

fn span_obj(log: &Log, s: &Span) -> Object {
    let b = log.get(s.begin.0);
    let mut o = obj([
        ("run", st(&s.run)),
        ("verb", st(s.verb.name())),
        ("type", st(b.map(Entry::type_token).unwrap_or_default())),
        ("object", st(&s.object)),
        ("actor", st(&s.actor)),
        ("rec", st(b.and_then(Entry::rec).unwrap_or(""))),
        ("begin", st(s.begin.0.to_string())),
        ("begin_us", us(s.begin.1)),
    ]);
    if let Some(p) = &s.parent {
        o.insert("parent".into(), st(p));
    }
    if let Some(prof) = b.and_then(|e| e.profile) {
        o.insert("profile".into(), st(prof.to_string()));
    }
    if let Some(note) = b.and_then(|e| e.get_str("title")) {
        o.insert("title".into(), st(note));
    }
    if let Some((a, t)) = s.end {
        o.insert("end".into(), st(a.to_string()));
        o.insert("end_us".into(), us(t));
        o.insert("use".into(), uses_obj(&s.uses));
        if let Some(w) = s.wall() {
            o.insert("wall".into(), st(w.to_string()));
        }
        if let Some(x) = &s.status {
            o.insert("status".into(), st(x));
        }
        if let Some(pool) = log.get(a).and_then(|e| e.get_str("pool")) {
            o.insert("pool".into(), st(pool));
        }
    }
    o
}

/// What the Now screen shows: open intervals with what is already known
/// about their use, recent closes and entries, refusals, gaps and seals.
fn now_view(log: &Log) -> Object {
    let (all, anomalies) = spans(log);
    let closed: Vec<&Span> = all.iter().filter(|s| s.end.is_some()).collect();

    // Meter samples: sample-phase entries with q, u and the pool they measure (scope).
    let samples: Vec<(&str, Time, Decimal, bool, &str)> = log
        .entries()
        .iter()
        .filter(|(_, e)| e.phase() == Phase::Sample)
        .filter_map(|(_, e)| {
            let (q, est) = decimal_str(e.get("q")?)?;
            Some((e.get_str("scope")?, e.time, q, est, e.get_str("u")?))
        })
        .collect();

    let mut open = vec![];
    for s in all.iter().filter(|s| s.end.is_none()) {
        let mut o = span_obj(log, s);
        let mut kids = BTreeMap::new();
        let mut n = 0;
        for c in closed.iter().filter(|c| c.parent.as_deref() == Some(s.run.as_str())) {
            n += 1;
            for (u, (d, e)) in &c.uses {
                sum_into(&mut kids, u, *d, *e);
            }
        }
        let mut meters: BTreeMap<String, BTreeMap<String, (Decimal, bool)>> = BTreeMap::new();
        for (scope, t, q, est, u) in &samples {
            if *t >= s.begin.1 {
                sum_into(meters.entry(scope.to_string()).or_default(), u, *q, *est);
            }
        }
        o.insert("children_closed".into(), Value::Int(n));
        o.insert("from_children".into(), uses_obj(&kids));
        o.insert("from_meters".into(), Value::Obj(meters.iter().map(|(k, m)| (k.clone(), uses_obj(m))).collect()));
        open.push(Value::Obj(o));
    }
    let recent_closed: Vec<Value> = {
        let mut c = closed.clone();
        c.sort_by_key(|s| s.end.map(|e| e.0));
        c.iter().rev().take(12).map(|s| Value::Obj(span_obj(log, s))).collect()
    };

    let mut next_ps: BTreeMap<String, i64> = BTreeMap::new();
    let mut refusals = vec![];
    let mut last_seal = None;
    let mut since_seal = 0i64;
    for (a, e) in log.entries() {
        if let (Some(rec), Some(ps)) = (e.rec(), e.get("ps").and_then(Value::as_int)) {
            let n = next_ps.entry(rec.to_string()).or_insert(1);
            *n = (*n).max(ps + 1);
        }
        if e.verb() == Verb::ACK {
            if let Some(why) = e.get_str("refused") {
                refusals.push(Value::Obj(obj([
                    ("addr", st(a.to_string())),
                    ("object", st(&e.object)),
                    ("rec", st(e.rec().unwrap_or(""))),
                    ("reason", st(why)),
                    ("time_us", us(e.time)),
                ])));
            }
        }
        if e.verb() == Verb::COMMIT && e.get_str("kind") == Some("seal") {
            last_seal = Some(obj([
                ("addr", st(a.to_string())),
                ("time_us", us(e.time)),
                ("chain", st(e.get_str("chain").unwrap_or(""))),
                ("n", e.get("n").cloned().unwrap_or(Value::Null)),
            ]));
            since_seal = 0;
        } else {
            since_seal += 1;
        }
    }
    let sample_list: Vec<Value> = {
        let v: Vec<Value> = log
            .entries()
            .iter()
            .filter(|(_, e)| e.phase() == Phase::Sample)
            .rev()
            .take(32)
            .map(|(a, e)| {
                let mut o = obj([
                    ("addr", st(a.to_string())),
                    ("object", st(&e.object)),
                    ("rec", st(e.rec().unwrap_or(""))),
                    ("time_us", us(e.time)),
                ]);
                for k in ["q", "u", "scope", "ps"] {
                    if let Some(x) = e.get(k) {
                        o.insert(k.into(), x.clone());
                    }
                }
                Value::Obj(o)
            })
            .collect();
        v.into_iter().rev().collect()
    };
    let recent: Vec<Value> = log
        .entries()
        .iter()
        .rev()
        .take(40)
        .map(|(a, e)| {
            Value::Obj(obj([
                ("addr", st(a.to_string())),
                ("line", st(e.to_line())),
                ("type", st(e.type_token())),
                ("object", st(&e.object)),
                ("rec", st(e.rec().unwrap_or(""))),
                ("time_us", us(e.time)),
                ("superseded", Value::Bool(log.is_superseded(*a))),
            ]))
        })
        .collect();
    let register: Vec<Value> = log
        .register()
        .all()
        .map(|d| {
            let mut o = obj([("kind", st(&d.kind)), ("id", st(&d.id)), ("addr", st(d.addr.to_string())), ("fields", Value::Obj(d.payload.clone()))]);
            if let Some(p) = &d.pack {
                o.insert("pack".into(), st(p));
            }
            Value::Obj(o)
        })
        .collect();
    let anomalies: Vec<Value> = anomalies
        .iter()
        .map(|a| match a {
            Anomaly::Reopen { run, at, prior } => st(format!("{run} begun again at {at} while open since {prior}")),
            Anomaly::Orphan { run, at } => st(format!("{run} ended at {at} with no begin")),
        })
        .collect();
    let gaps: Object = log.gaps().into_iter().map(|(r, v)| (r, Value::Arr(v.into_iter().map(Value::Int).collect()))).collect();
    let last = log.last_address();
    let sealed = log.get(last).is_some_and(|e| e.verb() == Verb::COMMIT && e.get_str("kind") == Some("seal"));

    obj([
        ("log", st(log.id())),
        ("roots", Value::Arr(log.roots().iter().map(st).collect())),
        ("kernel", st(stream_kernel::kernel_hash())),
        ("last", st(last.to_string())),
        ("entries", Value::Int(log.entries().len() as i64)),
        ("segment", Value::Int(last.seg as i64)),
        ("sealed", Value::Bool(sealed)),
        ("since_seal", Value::Int(since_seal)),
        ("last_seal", last_seal.map(Value::Obj).unwrap_or(Value::Null)),
        ("open", Value::Arr(open)),
        ("closed", Value::Arr(recent_closed)),
        ("recent", Value::Arr(recent)),
        ("samples", Value::Arr(sample_list)),
        ("refusals", Value::Arr(refusals.into_iter().rev().take(12).collect())),
        ("gaps", Value::Obj(gaps)),
        ("anomalies", Value::Arr(anomalies)),
        ("next_ps", Value::Obj(next_ps.into_iter().map(|(k, v)| (k, Value::Int(v))).collect())),
        ("register", Value::Arr(register)),
    ])
}

// ------------------------------------------------------------ WebAssembly

thread_local! {
    static SESSION: RefCell<Session> = RefCell::new(Session::new());
    static OUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Reserve `n` bytes for the caller to write a request into.
#[no_mangle]
pub extern "C" fn stream_alloc(n: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(n);
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// Release bytes reserved with [`stream_alloc`].
///
/// # Safety
/// `p` and `n` must come from one call to [`stream_alloc`].
#[no_mangle]
pub unsafe extern "C" fn stream_free(p: *mut u8, n: usize) {
    drop(Vec::from_raw_parts(p, 0, n));
}

/// Handle the UTF-8 request at `p..p+n`. Returns a pointer to the answer,
/// valid until the next call; its length is [`stream_out_len`].
///
/// # Safety
/// `p..p+n` must be readable memory holding the request.
#[no_mangle]
pub unsafe extern "C" fn stream_call(p: *const u8, n: usize) -> *const u8 {
    let req = std::str::from_utf8(std::slice::from_raw_parts(p, n)).unwrap_or("");
    let ans = SESSION.with(|s| s.borrow_mut().handle(req));
    OUT.with(|o| {
        let mut o = o.borrow_mut();
        *o = ans.into_bytes();
        o.as_ptr()
    })
}

#[no_mangle]
pub extern "C" fn stream_out_len() -> usize {
    OUT.with(|o| o.borrow().len())
}
