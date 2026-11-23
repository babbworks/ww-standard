//! The log: the only authoritative store.
//!
//! Append-only segments (`seg-000001.log`, …) plus content stored by hash
//! (`blobs/<sha256>`) for packs, held in a [`Store`]: a directory for the
//! CLI, memory for the browser. Everything else in this struct is derived
//! state, rebuilt by [`Log::open_store`].
//!
//! Single writer per store: one process appends at a time.

mod pack;
mod register;
mod store;

pub use pack::{Pack, PackError};
pub use register::{Decl, Register};
pub use store::{DirStore, MemStore, Store};

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io;
use std::path::Path;

use stream_kernel::json::Object;
use stream_kernel::{
    chain, entry::decimal_str, hex, kernel_hash, parse_preamble, preamble, sha256_hex, Address, Decimal, Entry,
    KernelError, Phase, Time, Type, Value, Verb, GENESIS_CHAIN,
};

/// The journal's answer to a submission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Receipt {
    Accepted(Address),
    /// A retry of something already accepted: the original address.
    Duplicate(Address),
    /// Refused. `at` is the journaled refusal (an ACK) when the submission
    /// carried a retry identity, so a retry gets the same answer.
    Rejected { at: Option<Address>, reason: String },
}

impl Receipt {
    pub fn addr(&self) -> Option<Address> {
        match self {
            Receipt::Accepted(a) | Receipt::Duplicate(a) => Some(*a),
            Receipt::Rejected { at, .. } => *at,
        }
    }
    pub fn is_accepted(&self) -> bool {
        matches!(self, Receipt::Accepted(_))
    }
}

#[derive(Debug)]
pub enum LogError {
    Io(io::Error),
    Kernel { at: String, err: KernelError },
    UnknownKernel(String),
    Corrupt(String),
    Exists,
}

impl std::fmt::Display for LogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogError::Io(e) => write!(f, "io: {e}"),
            LogError::Kernel { at, err } => write!(f, "{at}: {err}"),
            LogError::UnknownKernel(h) => write!(f, "log is written under kernel {h}, not this one ({})", kernel_hash()),
            LogError::Corrupt(m) => write!(f, "corrupt log: {m}"),
            LogError::Exists => write!(f, "a log already exists here"),
        }
    }
}
impl std::error::Error for LogError {}
impl From<io::Error> for LogError {
    fn from(e: io::Error) -> Self {
        LogError::Io(e)
    }
}

#[derive(Debug, Clone)]
struct Grant {
    to: String,
    may: String,
    pattern: Option<String>,
}

#[derive(Debug, Clone)]
struct RetryRec {
    addr: Address,
    digest: String,
    refused: Option<String>,
}

/// Options for a new log's genesis.
pub struct Genesis<'a> {
    pub log: &'a str,
    pub roots: &'a [&'a str],
    pub clock: &'a str,
    pub tz: &'a str,
    pub time: Time,
}

pub struct Log {
    store: Box<dyn Store>,
    log_id: String,
    roots: Vec<String>,
    entries: Vec<(Address, Entry)>,
    index: HashMap<Address, usize>,
    retry: HashMap<(String, String), RetryRec>,
    superseded: HashMap<Address, Address>,
    grants: Vec<Grant>,
    register: Register,
    seg: u32,
    seq: u32,
    sealed: bool,
    prev_chain: String,
}

fn retry_id(e: &Entry) -> Option<String> {
    if let Some(ps) = e.get("ps").and_then(Value::as_int) {
        return Some(format!("ps:{ps}"));
    }
    e.get_str("rk").map(|rk| format!("rk:{rk}"))
}

fn glob(pattern: &str, s: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(prefix) => s.starts_with(prefix),
        None => pattern == s,
    }
}

impl Log {
    // ------------------------------------------------------------ lifecycle

    /// Create a log in a directory: preamble, then genesis.
    pub fn init(dir: &Path, g: Genesis) -> Result<Log, LogError> {
        fs::create_dir_all(dir.join("blobs"))?;
        Log::init_store(Box::new(DirStore::new(dir)), g)
    }

    /// Open a log in a directory.
    pub fn open(dir: &Path) -> Result<Log, LogError> {
        Log::open_store(Box::new(DirStore::new(dir)))
    }

    /// Create a log in any store: preamble, then genesis.
    pub fn init_store(mut store: Box<dyn Store>, g: Genesis) -> Result<Log, LogError> {
        if store.segment(1)?.is_some() {
            return Err(LogError::Exists);
        }
        let mut p = Object::new();
        p.insert("clock".into(), Value::str(g.clock));
        p.insert("kind".into(), Value::str("genesis"));
        p.insert("log".into(), Value::str(g.log));
        p.insert("rec".into(), Value::str(g.roots[0]));
        p.insert("root".into(), Value::Arr(g.roots.iter().map(|r| Value::str(r)).collect()));
        p.insert("tz".into(), Value::str(g.tz));
        let genesis = Entry::new(g.time, Type::new(Phase::Instant, Verb::OPEN), &format!("log:{}", g.log), p);
        genesis.check().map_err(|err| LogError::Kernel { at: "genesis".into(), err })?;
        store.append(1, format!("{}\n{}\n", preamble(), genesis.to_line()).as_bytes())?;
        Log::open_store(store)
    }

    /// Open a log in any store and rebuild all derived state from its segments.
    pub fn open_store(store: Box<dyn Store>) -> Result<Log, LogError> {
        let mut log = Log {
            store,
            log_id: String::new(),
            roots: vec![],
            entries: vec![],
            index: HashMap::new(),
            retry: HashMap::new(),
            superseded: HashMap::new(),
            grants: vec![],
            register: Register::default(),
            seg: 0,
            seq: 0,
            sealed: true,
            prev_chain: GENESIS_CHAIN.to_string(),
        };
        let mut n = 1;
        while let Some(bytes) = log.store.segment(n)? {
            let text = String::from_utf8(bytes).map_err(|_| LogError::Corrupt(format!("seg {n} is not UTF-8")))?;
            log.seg = n;
            log.seq = 0;
            log.sealed = false;
            for (i, line) in text.lines().enumerate() {
                if n == 1 && i == 0 {
                    let h = parse_preamble(line).map_err(|err| LogError::Kernel { at: "line 1".into(), err })?;
                    if h != kernel_hash() {
                        return Err(LogError::UnknownKernel(h.to_string()));
                    }
                    continue;
                }
                let e = Entry::parse(line).map_err(|err| LogError::Kernel { at: format!("seg {n} line {}", i + 1), err })?;
                log.seq += 1;
                let addr = Address { seg: n, seq: log.seq };
                log.apply(addr, e)?;
            }
            n += 1;
        }
        if log.entries.is_empty() {
            return Err(LogError::Corrupt("no genesis".into()));
        }
        Ok(log)
    }

    // ------------------------------------------------------------- reading

    /// Where the log is stored, for messages.
    pub fn location(&self) -> String {
        self.store.location()
    }
    /// The store holding the log's bytes.
    pub fn store(&self) -> &dyn Store {
        self.store.as_ref()
    }
    pub fn id(&self) -> &str {
        &self.log_id
    }
    pub fn roots(&self) -> &[String] {
        &self.roots
    }
    pub fn entries(&self) -> &[(Address, Entry)] {
        &self.entries
    }
    pub fn get(&self, a: Address) -> Option<&Entry> {
        self.index.get(&a).map(|&i| &self.entries[i].1)
    }
    pub fn is_superseded(&self, a: Address) -> bool {
        self.superseded.contains_key(&a)
    }
    pub fn superseded_by(&self, a: Address) -> Option<Address> {
        self.superseded.get(&a).copied()
    }
    pub fn register(&self) -> &Register {
        &self.register
    }
    pub fn last_address(&self) -> Address {
        self.entries.last().map(|(a, _)| *a).expect("genesis exists")
    }
    /// The log's own recorder, used for its bookkeeping entries.
    pub fn system_rec(&self) -> String {
        format!("log:{}", self.log_id)
    }

    /// Producer sequence numbers never received, per recorder. Rejections
    /// are journaled, so they count as received.
    pub fn gaps(&self) -> BTreeMap<String, Vec<i64>> {
        let mut seen: BTreeMap<String, Vec<i64>> = BTreeMap::new();
        for (_, e) in &self.entries {
            if let (Some(rec), Some(ps)) = (e.rec(), e.get("ps").and_then(Value::as_int)) {
                seen.entry(rec.to_string()).or_default().push(ps);
            }
        }
        let mut out = BTreeMap::new();
        for (rec, mut v) in seen {
            v.sort_unstable();
            v.dedup();
            let max = *v.last().unwrap_or(&0);
            let missing: Vec<i64> = (1..=max).filter(|p| v.binary_search(p).is_err()).collect();
            if !missing.is_empty() {
                out.insert(rec, missing);
            }
        }
        out
    }

    // ------------------------------------------------------------- writing

    /// Submit one entry as `submitter`. The journal sets `rec`.
    pub fn submit(&mut self, submitter: &str, mut e: Entry, now: Time) -> Result<Receipt, LogError> {
        if let Some(r) = e.rec() {
            if r != submitter {
                return Ok(Receipt::Rejected { at: None, reason: format!("rec {r:?} is not the submitter {submitter:?}") });
            }
        }
        e.payload.insert("rec".into(), Value::str(submitter));
        let digest = hex(&e.digest());
        let rid = retry_id(&e);
        if let Some(rid) = &rid {
            if let Some(r) = self.retry.get(&(submitter.to_string(), rid.clone())) {
                if r.digest != digest {
                    return Ok(Receipt::Rejected {
                        at: None,
                        reason: format!("{rid} already used by a different entry at {}", r.addr),
                    });
                }
                return Ok(match &r.refused {
                    Some(reason) => Receipt::Rejected { at: Some(r.addr), reason: reason.clone() },
                    None => Receipt::Duplicate(r.addr),
                });
            }
        }
        if let Err(reason) = self.validate(submitter, &e) {
            let Some(rid) = rid else {
                return Ok(Receipt::Rejected { at: None, reason });
            };
            let mut p = Object::new();
            p.insert("digest".into(), Value::str(&digest));
            p.insert("rec".into(), Value::str(submitter));
            p.insert("refused".into(), Value::Str(reason.clone()));
            let (k, v) = rid.split_once(':').expect("rid has a kind");
            p.insert(k.into(), e.get(k).cloned().unwrap_or(Value::str(v)));
            let ack = Entry::new(now, Type::new(Phase::Instant, Verb::ACK), &e.object, p);
            let at = self.write(ack, now)?;
            return Ok(Receipt::Rejected { at: Some(at), reason });
        }
        let at = self.write(e, now)?;
        Ok(Receipt::Accepted(at))
    }

    /// Parse and submit a line.
    pub fn submit_line(&mut self, submitter: &str, line: &str, now: Time) -> Result<Receipt, LogError> {
        match Entry::parse(line) {
            Ok(e) => self.submit(submitter, e, now),
            Err(err) => Ok(Receipt::Rejected { at: None, reason: err.to_string() }),
        }
    }

    /// Store a pack's bytes and adopt it with one BIND entry.
    pub fn adopt(&mut self, submitter: &str, pack_bytes: &[u8], extra: Object, now: Time) -> Result<Receipt, LogError> {
        let pack = match Pack::parse(pack_bytes) {
            Ok(p) => p,
            Err(e) => return Ok(Receipt::Rejected { at: None, reason: e.to_string() }),
        };
        self.store.put_blob(&pack.hash, pack_bytes)?;
        let mut p = extra;
        p.insert("name".into(), Value::str(&pack.name));
        p.insert("pack".into(), Value::Str(format!("sha256:{}", pack.hash)));
        self.submit(submitter, Entry::new(now, Type::new(Phase::Instant, Verb::BIND), "log", p), now)
    }

    /// Seal the open segment. The next accepted entry opens a new one.
    pub fn seal(&mut self, now: Time) -> Result<Address, LogError> {
        if self.sealed {
            return Err(LogError::Corrupt("no open segment to seal".into()));
        }
        let body = self.store.segment(self.seg)?.unwrap_or_default();
        let body_hash = sha256_hex(&body);
        let ch = chain(&self.prev_chain, &body_hash);
        let mut p = Object::new();
        p.insert("chain".into(), Value::Str(ch));
        p.insert("kind".into(), Value::str("seal"));
        p.insert("n".into(), Value::Int(self.seq as i64));
        p.insert("rec".into(), Value::Str(self.system_rec()));
        p.insert("sha256".into(), Value::Str(body_hash));
        let e = Entry::new(now, Type::new(Phase::Instant, Verb::COMMIT), &format!("seg-{:06}", self.seg), p);
        self.append_raw(e)
    }

    /// Recompute every seal and the chain. Returns the problems found.
    pub fn verify(&self) -> Vec<String> {
        let mut problems = vec![];
        let mut prev = GENESIS_CHAIN.to_string();
        let mut n = 1;
        loop {
            let bytes = match self.store.segment(n) {
                Ok(Some(b)) => b,
                Ok(None) => break,
                Err(e) => {
                    problems.push(format!("seg {n}: {e}"));
                    break;
                }
            };
            let text = String::from_utf8_lossy(&bytes);
            let lines: Vec<&str> = text.lines().collect();
            let first_entry = if n == 1 { 1 } else { 0 };
            if n > 1 {
                match lines.first().and_then(|l| Entry::parse(l).ok()) {
                    Some(o) if o.verb() == Verb::OPEN && o.get_str("prev") == Some(prev.as_str()) => {}
                    _ => problems.push(format!("seg {n}: does not open with prev = previous chain")),
                }
            }
            for (i, l) in lines.iter().enumerate().skip(first_entry) {
                if let Err(err) = Entry::parse(l).and_then(|e| e.check()) {
                    problems.push(format!("seg {n} line {}: {err}", i + 1));
                }
            }
            if let Some(last) = lines.last().and_then(|l| Entry::parse(l).ok()) {
                if last.verb() == Verb::COMMIT && last.get_str("kind") == Some("seal") {
                    let cut = bytes.len() - lines.last().unwrap().len() - 1;
                    let body_hash = sha256_hex(&bytes[..cut]);
                    if Some(body_hash.as_str()) != last.get_str("sha256") {
                        problems.push(format!("seg {n}: body hash mismatch"));
                    }
                    let entries = lines.len() - 1 - first_entry;
                    if last.get("n").and_then(Value::as_int) != Some(entries as i64) {
                        problems.push(format!("seg {n}: entry count mismatch"));
                    }
                    let ch = chain(&prev, last.get_str("sha256").unwrap_or(""));
                    if Some(ch.as_str()) != last.get_str("chain") {
                        problems.push(format!("seg {n}: chain mismatch"));
                    }
                    prev = last.get_str("chain").unwrap_or("").to_string();
                }
            }
            n += 1;
        }
        problems
    }

    // ------------------------------------------------------------ internals

    fn write(&mut self, e: Entry, now: Time) -> Result<Address, LogError> {
        if self.sealed {
            let mut p = Object::new();
            p.insert("kind".into(), Value::str("segment"));
            p.insert("prev".into(), Value::Str(self.prev_chain.clone()));
            p.insert("rec".into(), Value::Str(self.system_rec()));
            let open = Entry::new(now, Type::new(Phase::Instant, Verb::OPEN), &format!("seg-{:06}", self.seg + 1), p);
            self.seg += 1;
            self.seq = 0;
            self.sealed = false;
            self.append_raw(open)?;
        }
        self.append_raw(e)
    }

    fn append_raw(&mut self, e: Entry) -> Result<Address, LogError> {
        e.check().map_err(|err| LogError::Kernel { at: "append".into(), err })?;
        let mut line = e.to_line();
        line.push('\n');
        self.store.append(self.seg, line.as_bytes())?;
        self.seq += 1;
        let addr = Address { seg: self.seg, seq: self.seq };
        self.apply(addr, e)?;
        Ok(addr)
    }

    /// Fold one committed entry into the derived state. Used both when
    /// appending and when rebuilding, so the two can never disagree.
    fn apply(&mut self, addr: Address, e: Entry) -> Result<(), LogError> {
        let rec = e.rec().unwrap_or("").to_string();
        match (e.verb(), e.get_str("kind")) {
            (Verb::OPEN, Some("genesis")) => {
                self.log_id = e.get_str("log").unwrap_or("").to_string();
                self.roots = e.get("root").and_then(Value::as_arr).unwrap_or(&[]).iter().filter_map(|v| v.as_str().map(String::from)).collect();
            }
            (Verb::COMMIT, Some("seal")) => {
                self.prev_chain = e.get_str("chain").unwrap_or("").to_string();
                self.sealed = true;
            }
            (Verb::GRANT, _) => self.grants.push(Grant {
                to: e.get_str("to").unwrap_or("").into(),
                may: e.get_str("may").unwrap_or("").into(),
                pattern: e.get_str("for").map(String::from),
            }),
            (Verb::REVOKE, _) => {
                let (to, may) = (e.get_str("to").unwrap_or(""), e.get_str("may").unwrap_or(""));
                let pat = e.get_str("for");
                self.grants.retain(|g| !(g.to == to && g.may == may && g.pattern.as_deref() == pat));
            }
            (Verb::CREATE, _) => self.register.declare(&e, addr, None),
            (Verb::BIND, _) => {
                if let Some(h) = e.get_str("pack").and_then(|p| p.strip_prefix("sha256:")) {
                    let bytes = self.store.blob(h)?
                        .ok_or_else(|| LogError::Corrupt(format!("{addr}: pack blob {h} is missing")))?;
                    let pack = Pack::parse(&bytes).map_err(|e| LogError::Corrupt(format!("{addr}: {e}")))?;
                    for d in &pack.decls {
                        self.register.declare(d, addr, Some(&pack.name));
                    }
                }
            }
            (Verb::AMEND, _) => {
                if let Some(r) = e.get_str("ref").and_then(Address::parse) {
                    self.superseded.insert(r, addr);
                }
                self.register.amend(&e, addr, self.index.get(&Address::parse(e.get_str("ref").unwrap_or("")).unwrap_or(addr)).map(|&i| &self.entries[i].1));
            }
            _ => {}
        }
        if let Some(rid) = retry_id(&e) {
            let refused = (e.verb() == Verb::ACK).then(|| e.get_str("refused").map(String::from)).flatten();
            let digest = if refused.is_some() { e.get_str("digest").unwrap_or("").to_string() } else { hex(&e.digest()) };
            if e.verb() != Verb::ACK || refused.is_some() {
                self.retry.insert((rec, rid), RetryRec { addr, digest, refused });
            }
        }
        self.index.insert(addr, self.entries.len());
        self.entries.push((addr, e));
        Ok(())
    }

    fn is_root(&self, who: &str) -> bool {
        self.roots.iter().any(|r| r == who)
    }

    fn has_grant(&self, who: &str, may: &str, target: Option<&str>) -> bool {
        self.grants.iter().any(|g| {
            g.to == who && g.may == may && match (target, &g.pattern) {
                (Some(t), Some(p)) => glob(p, t),
                (None, _) => true,
                (Some(_), None) => false,
            }
        })
    }

    fn admin(&self, who: &str) -> bool {
        self.is_root(who) || self.has_grant(who, "admin", None)
    }

    /// Would `rec` be allowed to submit `e` now? A dry run of the checks
    /// [`Log::submit`] makes, without writing anything.
    pub fn check_entry(&self, rec: &str, e: &Entry) -> Result<(), String> {
        if let Some(r) = e.rec() {
            if r != rec {
                return Err(format!("rec {r:?} is not the submitter {rec:?}"));
            }
        }
        let mut e = e.clone();
        e.payload.insert("rec".into(), Value::str(rec));
        self.validate(rec, &e)
    }

    /// Everything a submitted entry must satisfy beyond the kernel grammar.
    fn validate(&self, rec: &str, e: &Entry) -> Result<(), String> {
        e.check().map_err(|err| err.to_string())?;
        let v = e.verb();

        // Entries only the journal itself writes.
        if matches!(v, Verb::OPEN | Verb::ACK) || (v == Verb::COMMIT && e.get_str("kind") == Some("seal")) {
            return Err(format!("{} entries are written by the log, not submitted", v.name()));
        }

        // Authority.
        let needs_admin = matches!(v, Verb::CREATE | Verb::BIND | Verb::GRANT | Verb::REVOKE | Verb::COMMIT);
        if needs_admin && !self.admin(rec) {
            return Err(format!("{rec} may not {}", v.name()));
        }
        if let Some(actor) = e.get_str("actor") {
            if actor != rec && !self.is_root(rec) && !self.has_grant(rec, "record", Some(actor)) {
                return Err(format!("{rec} may not record for {actor}"));
            }
        }

        // Quantities.
        if let Some(q) = e.get("q") {
            decimal_str(q).ok_or("q must be a decimal string")?;
        }
        if let Some(u) = e.get("use") {
            let m = u.as_obj().filter(|m| !m.is_empty()).ok_or("use must be a non-empty object")?;
            for (code, val) in m {
                decimal_str(val).ok_or_else(|| format!("use.{code} must be a decimal string"))?;
            }
        }
        if let Some(legs) = e.get("legs") {
            check_legs(legs)?;
        }

        match v {
            Verb::GRANT => match e.get_str("may") {
                Some("admin") => {}
                Some("record") if e.get_str("for").is_some() => {}
                _ => return Err("may must be admin, or record with for".into()),
            },
            Verb::REVOKE => {
                let pat = e.get_str("for");
                let found = self.grants.iter().any(|g| {
                    Some(g.to.as_str()) == e.get_str("to") && Some(g.may.as_str()) == e.get_str("may") && g.pattern.as_deref() == pat
                });
                if !found {
                    return Err("no such grant to revoke".into());
                }
            }
            Verb::AMEND => {
                let r = Address::parse(e.get_str("ref").unwrap_or("")).ok_or("bad ref")?;
                let target = self.get(r).ok_or_else(|| format!("{r} is not a committed address"))?;
                if let Some(by) = self.superseded.get(&r) {
                    return Err(format!("{r} already superseded by {by}; amend the latest version"));
                }
                if matches!(target.verb(), Verb::OPEN | Verb::COMMIT | Verb::ACK) {
                    return Err(format!("{} entries cannot be amended", target.verb().name()));
                }
                if target.rec() != Some(rec) && !self.admin(rec) {
                    return Err(format!("{rec} may not amend an entry recorded by {}", target.rec().unwrap_or("?")));
                }
            }
            Verb::CREATE => {
                if let Some(existing) = self.register.get(e.get_str("kind").unwrap_or(""), &e.object) {
                    if existing.payload != register::strip_rec(&e.payload) {
                        return Err(format!("{} {} is already declared differently at {}; use AMEND", existing.kind, e.object, existing.addr));
                    }
                }
            }
            Verb::BIND => {
                if let Some(h) = e.get_str("pack").and_then(|p| p.strip_prefix("sha256:")) {
                    let bytes = self.store.blob(h).ok().flatten().ok_or_else(|| format!("pack {h} is not stored"))?;
                    let pack = Pack::parse(&bytes).map_err(|e| e.to_string())?;
                    for d in &pack.decls {
                        let kind = d.get_str("kind").unwrap_or("");
                        if let Some(existing) = self.register.get(kind, &d.object) {
                            if existing.payload != register::strip_rec(&d.payload) {
                                return Err(format!("pack {} redefines {kind} {} (declared at {})", pack.name, d.object, existing.addr));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// A posting's legs balance per (book, unit), each leg a non-zero decimal.
fn check_legs(legs: &Value) -> Result<(), String> {
    let legs = legs.as_arr().ok_or("legs must be a list")?;
    if legs.len() < 2 {
        return Err("a posting needs at least two legs".into());
    }
    let mut sums: BTreeMap<(String, String), Decimal> = BTreeMap::new();
    for (i, leg) in legs.iter().enumerate() {
        let o = leg.as_obj().ok_or_else(|| format!("leg {i} is not an object"))?;
        o.get("a").and_then(Value::as_str).ok_or_else(|| format!("leg {i} needs a"))?;
        let u = o.get("u").and_then(Value::as_str).ok_or_else(|| format!("leg {i} needs u"))?;
        let q = o.get("q").and_then(Value::as_str).ok_or_else(|| format!("leg {i} needs q"))?;
        let q = Decimal::parse(q).map_err(|_| format!("leg {i} q is not a decimal"))?;
        if q.is_zero() {
            return Err(format!("leg {i} has zero quantity"));
        }
        let book = o.get("book").and_then(Value::as_str).unwrap_or("-").to_string();
        let s = sums.entry((book, u.to_string())).or_insert(Decimal::ZERO);
        *s = s.checked_add(q).ok_or("overflow")?;
    }
    let bad: Vec<String> = sums.iter().filter(|(_, s)| !s.is_zero()).map(|((b, u), s)| format!("{b}/{u} {s}")).collect();
    if bad.is_empty() {
        Ok(())
    } else {
        Err(format!("unbalanced: {}", bad.join(", ")))
    }
}
