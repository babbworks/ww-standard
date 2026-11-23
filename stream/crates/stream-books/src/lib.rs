//! The Books and the built-in views.
//!
//! Every function here is a pure fold over the log up to an address: it
//! reads entries and nothing else. The same functions will run inside the
//! WebAssembly view host once it exists; they hold no special privilege.

use std::collections::BTreeMap;

use stream_kernel::json::Object;
use stream_kernel::{entry::decimal_str, sha256_hex, Address, Decimal, Entry, Phase, Time, Type, Value, Verb};
use stream_log::Log;

/// (book, account, unit)
pub type Key = (String, String, String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Balance {
    pub q: Decimal,
    /// True if any contribution was an estimate (`~`).
    pub estimated: bool,
    /// The entries this balance came from.
    pub from: Vec<Address>,
}

fn add(t: &mut BTreeMap<Key, Balance>, key: Key, q: Decimal, est: bool, at: Address) {
    let b = t.entry(key).or_insert(Balance { q: Decimal::ZERO, estimated: false, from: vec![] });
    b.q = b.q.checked_add(q).expect("balance overflow");
    b.estimated |= est;
    if b.from.last() != Some(&at) {
        b.from.push(at);
    }
}

fn within(a: Address, upto: Option<Address>) -> bool {
    upto.is_none_or(|u| a <= u)
}

/// Balances per (book, account, unit).
///
/// * Explicit movements: any entry with `legs` that has not been superseded.
///   An AMEND with legs replaces its target; an AMEND without legs reverses it.
/// * Implied movements: an entry in the End phase with `use` and `pool` moves
///   each quantity from the pool to the activity (`account`, else the object).
pub fn balances(log: &Log, upto: Option<Address>) -> BTreeMap<Key, Balance> {
    let mut t = BTreeMap::new();
    for (a, e) in log.entries() {
        if !within(*a, upto) || log.superseded_by(*a).is_some_and(|by| within(by, upto)) {
            continue;
        }
        if let Some(legs) = e.get("legs").and_then(Value::as_arr) {
            for leg in legs {
                let Some(o) = leg.as_obj() else { continue };
                let get = |k: &str| o.get(k).and_then(Value::as_str);
                if let (Some(acct), Some(q), Some(u)) = (get("a"), get("q"), get("u")) {
                    if let Ok(q) = Decimal::parse(q) {
                        let book = get("book").unwrap_or("-");
                        add(&mut t, (book.into(), acct.into(), u.into()), q, false, *a);
                    }
                }
            }
        }
        if e.phase() == Phase::End {
            if let (Some(uses), Some(pool)) = (e.get("use").and_then(Value::as_obj), e.get_str("pool")) {
                let book = e.get_str("book").unwrap_or("-");
                let activity = e.get_str("account").unwrap_or(&e.object);
                for (unit, v) in uses {
                    if let Some((q, est)) = decimal_str(v) {
                        add(&mut t, (book.into(), pool.into(), unit.clone()), -q, est, *a);
                        add(&mut t, (book.into(), activity.into(), unit.clone()), q, est, *a);
                    }
                }
            }
        }
    }
    t
}

/// The balance table of one book, in canonical text: one line per account
/// and unit, sorted, `account<TAB>unit<TAB>[~]quantity`. Zero balances are kept.
pub fn balance_table(t: &BTreeMap<Key, Balance>, book: &str) -> String {
    let mut s = String::new();
    for ((b, acct, unit), bal) in t {
        if b == book {
            s.push_str(&format!("{acct}\t{unit}\t{}{}\n", if bal.estimated { "~" } else { "" }, bal.q));
        }
    }
    s
}

/// SHA-256 of a book's balance table up to an address: what a Close commits to.
pub fn balances_hash(log: &Log, book: &str, upto: Address) -> String {
    sha256_hex(balance_table(&balances(log, Some(upto)), book).as_bytes())
}

/// The COMMIT entry that closes `book` up to `upto`.
pub fn close_entry(log: &Log, book: &str, upto: Address, time: Time) -> Entry {
    let mut p = Object::new();
    p.insert("balances".into(), Value::Str(balances_hash(log, book, upto)));
    p.insert("book".into(), Value::str(book));
    p.insert("kind".into(), Value::str("close"));
    p.insert("upto".into(), Value::Str(upto.to_string()));
    Entry::new(time, Type::new(Phase::Instant, Verb::COMMIT), &format!("book:{book}"), p)
}

/// Recompute every Close in the log. Returns the closes whose hash no longer matches.
pub fn verify_closes(log: &Log) -> Vec<Address> {
    log.entries()
        .iter()
        .filter(|(_, e)| e.verb() == Verb::COMMIT && e.get_str("kind") == Some("close"))
        .filter(|(_, e)| {
            let upto = e.get_str("upto").and_then(Address::parse);
            let book = e.get_str("book").unwrap_or("");
            upto.map(|u| Some(balances_hash(log, book, u).as_str()) != e.get_str("balances")).unwrap_or(true)
        })
        .map(|(a, _)| *a)
        .collect()
}

// ------------------------------------------------------------------ spans

#[derive(Debug, Clone)]
pub struct Span {
    pub run: String,
    pub verb: Verb,
    pub object: String,
    pub actor: String,
    pub parent: Option<String>,
    pub begin: (Address, Time),
    pub end: Option<(Address, Time)>,
    pub status: Option<String>,
    pub uses: BTreeMap<String, (Decimal, bool)>,
}

impl Span {
    /// Wall time in seconds, exact.
    pub fn wall(&self) -> Option<Decimal> {
        let (_, e) = self.end?;
        Some(Decimal::from_parts(e.as_micros() - self.begin.1.as_micros(), 6).normalized())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Anomaly {
    /// A second begin on an interval that was already open.
    Reopen { run: String, at: Address, prior: Address },
    /// An end with no matching begin.
    Orphan { run: String, at: Address },
}

fn pair_key(e: &Entry) -> String {
    match e.get_str("run") {
        Some(r) => format!("{}:{r}", e.verb().name()),
        None => format!("{}:{}|{}", e.verb().name(), e.object, e.actor().unwrap_or("")),
    }
}

/// Pair begin and end entries into spans, in occurrence order. Pairing is by
/// `run` when present, otherwise by (verb, object, actor), never by object alone.
pub fn spans(log: &Log) -> (Vec<Span>, Vec<Anomaly>) {
    let mut rows: Vec<&(Address, Entry)> = log
        .entries()
        .iter()
        .filter(|(_, e)| matches!(e.phase(), Phase::Begin | Phase::End))
        .collect();
    rows.sort_by_key(|(a, e)| (e.time, *a));
    let mut open: BTreeMap<String, Span> = BTreeMap::new();
    let mut done = vec![];
    let mut anomalies = vec![];
    for (a, e) in rows {
        let key = pair_key(e);
        let run = e.get_str("run").unwrap_or(&key).to_string();
        if e.phase() == Phase::Begin {
            if let Some(prior) = open.get(&key) {
                anomalies.push(Anomaly::Reopen { run: run.clone(), at: *a, prior: prior.begin.0 });
            }
            open.insert(key, Span {
                run,
                verb: e.verb(),
                object: e.object.clone(),
                actor: e.actor().unwrap_or("").into(),
                parent: e.get_str("parent").map(String::from),
                begin: (*a, e.time),
                end: None,
                status: None,
                uses: BTreeMap::new(),
            });
        } else if let Some(mut s) = open.remove(&key) {
            s.end = Some((*a, e.time));
            s.status = e.get_str("status").map(String::from);
            if let Some(u) = e.get("use").and_then(Value::as_obj) {
                for (k, v) in u {
                    if let Some(d) = decimal_str(v) {
                        s.uses.insert(k.clone(), d);
                    }
                }
            }
            done.push(s);
        } else {
            anomalies.push(Anomaly::Orphan { run, at: *a });
        }
    }
    done.extend(open.into_values());
    (done, anomalies)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Attribution {
    pub wall: Decimal,
    pub children: Vec<String>,
    /// Conserved quantity left after the recorded children: work below the recorded depth.
    pub remainder: BTreeMap<String, (Decimal, bool)>,
    /// Quantities the children claim beyond the parent's use.
    pub over: BTreeMap<String, (Decimal, bool)>,
    /// CPU seconds (`xs`) over wall seconds.
    pub utilisation: Option<f64>,
    /// Total child wall time over the parent's wall time.
    pub parallelism: Option<f64>,
}

/// For each closed span: what its children account for, and what is left.
/// `conserved` names the quantity codes that add up (energy, CPU, tokens…).
pub fn attribution(log: &Log, conserved: &[&str]) -> BTreeMap<String, Attribution> {
    let (spans, _) = spans(log);
    let closed: Vec<&Span> = spans.iter().filter(|s| s.end.is_some()).collect();
    let mut out = BTreeMap::new();
    for p in &closed {
        let kids: Vec<&&Span> = closed.iter().filter(|c| c.parent.as_deref() == Some(p.run.as_str())).collect();
        let wall = p.wall().unwrap_or(Decimal::ZERO);
        let mut a = Attribution {
            wall,
            children: kids.iter().map(|k| k.run.clone()).collect(),
            remainder: BTreeMap::new(),
            over: BTreeMap::new(),
            utilisation: None,
            parallelism: None,
        };
        if !wall.is_zero() {
            if let Some((xs, _)) = p.uses.get("xs") {
                a.utilisation = Some((xs.to_f64() / wall.to_f64() * 1000.0).round() / 1000.0);
            }
            if !kids.is_empty() {
                let total: f64 = kids.iter().filter_map(|k| k.wall()).map(Decimal::to_f64).sum();
                a.parallelism = Some((total / wall.to_f64() * 1000.0).round() / 1000.0);
            }
        }
        for (code, (total, est)) in &p.uses {
            if !conserved.contains(&code.as_str()) {
                continue;
            }
            let mut rem = *total;
            let mut est = *est;
            for k in &kids {
                if let Some((q, e)) = k.uses.get(code) {
                    rem = rem.checked_sub(*q).expect("overflow");
                    est |= *e;
                }
            }
            if rem < Decimal::ZERO {
                a.over.insert(code.clone(), (rem, est));
            } else {
                a.remainder.insert(code.clone(), (rem, est));
            }
        }
        out.insert(p.run.clone(), a);
    }
    out
}

/// References to objects the Register has never declared (accepted, but flagged).
pub fn undeclared(log: &Log, kinds: &[&str]) -> Vec<(Address, String)> {
    let reg = log.register();
    log.entries()
        .iter()
        .filter(|(_, e)| !e.verb().is_bootstrap() && e.verb() != Verb::ACK)
        .filter(|(_, e)| !kinds.iter().any(|k| reg.get(k, &e.object).is_some()))
        .map(|(a, e)| (*a, e.object.clone()))
        .collect()
}
