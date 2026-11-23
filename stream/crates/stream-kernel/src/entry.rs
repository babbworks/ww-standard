//! Entries: the one canonical line form, and the kernel's structural checks.

use crate::hash::{is_hex64, sha256};
use crate::json::{object_to_string, parse_object, Object, Value};
use crate::verb::{Phase, Type, Verb};
use crate::Decimal;
use alloc::string::{String, ToString};
use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelError {
    Preamble,
    Fields,
    Time,
    Type(String),
    Object,
    Payload(String),
    NotCanonical,
    Missing(&'static str),
    Invalid(String),
}

impl fmt::Display for KernelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KernelError::Preamble => write!(f, "not a STREAM/1 preamble"),
            KernelError::Fields => write!(f, "an entry has four space-separated fields"),
            KernelError::Time => write!(f, "bad time"),
            KernelError::Type(t) => write!(f, "unknown type {t:?}"),
            KernelError::Object => write!(f, "bad object"),
            KernelError::Payload(e) => write!(f, "bad payload: {e}"),
            KernelError::NotCanonical => write!(f, "entry is not in canonical form"),
            KernelError::Missing(k) => write!(f, "payload requires {k:?}"),
            KernelError::Invalid(e) => write!(f, "{e}"),
        }
    }
}

/// Unix seconds plus microseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Time {
    pub secs: u64,
    pub micros: u32,
}

impl Time {
    pub fn secs(secs: u64) -> Time {
        Time { secs, micros: 0 }
    }
    pub fn as_micros(self) -> i128 {
        self.secs as i128 * 1_000_000 + self.micros as i128
    }
    pub fn parse(s: &str) -> Result<Time, KernelError> {
        let (a, b) = match s.split_once('.') {
            Some((a, b)) => (a, Some(b)),
            None => (s, None),
        };
        if a.is_empty() || !a.bytes().all(|c| c.is_ascii_digit()) || (a.len() > 1 && a.starts_with('0')) {
            return Err(KernelError::Time);
        }
        let secs = a.parse().map_err(|_| KernelError::Time)?;
        let micros = match b {
            None => 0,
            Some(f) => {
                if f.is_empty() || f.len() > 6 || !f.bytes().all(|c| c.is_ascii_digit()) || f.ends_with('0') {
                    return Err(KernelError::Time);
                }
                let mut m: u32 = f.parse().map_err(|_| KernelError::Time)?;
                for _ in f.len()..6 {
                    m *= 10;
                }
                m
            }
        };
        Ok(Time { secs, micros })
    }
}

impl fmt::Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.micros == 0 {
            write!(f, "{}", self.secs)
        } else {
            let frac = alloc::format!("{:06}", self.micros);
            write!(f, "{}.{}", self.secs, frac.trim_end_matches('0'))
        }
    }
}

/// A committed entry's position: segment and sequence, both from 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Address {
    pub seg: u32,
    pub seq: u32,
}

impl Address {
    pub fn parse(s: &str) -> Option<Address> {
        let (a, b) = s.split_once(':')?;
        let seg: u32 = a.parse().ok()?;
        let seq: u32 = b.parse().ok()?;
        (seg >= 1 && seq >= 1 && a == seg.to_string() && b == seq.to_string()).then_some(Address { seg, seq })
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.seg, self.seq)
    }
}

/// One act, or one end of an act.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub time: Time,
    pub profile: Option<char>,
    pub ty: Type,
    pub object: String,
    pub payload: Object,
}

fn object_ok(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| !c.is_whitespace() && !c.is_control())
}

impl Entry {
    pub fn new(time: Time, ty: Type, object: &str, payload: Object) -> Entry {
        Entry { time, profile: None, ty, object: object.to_string(), payload }
    }

    pub fn verb(&self) -> Verb {
        self.ty.verb()
    }
    pub fn phase(&self) -> Phase {
        self.ty.phase()
    }

    pub fn get(&self, k: &str) -> Option<&Value> {
        self.payload.get(k)
    }
    pub fn get_str(&self, k: &str) -> Option<&str> {
        self.payload.get(k).and_then(Value::as_str)
    }
    pub fn rec(&self) -> Option<&str> {
        self.get_str("rec")
    }
    /// Who acted: `actor` if stated, else the recorder.
    pub fn actor(&self) -> Option<&str> {
        self.get_str("actor").or_else(|| self.rec())
    }

    /// The type token: `[P:]VERB[phase]`.
    pub fn type_token(&self) -> String {
        match self.profile {
            Some(p) => alloc::format!("{}:{}", p, self.ty),
            None => self.ty.to_string(),
        }
    }

    /// The canonical line, without the trailing newline.
    pub fn to_line(&self) -> String {
        alloc::format!("{} {} {} {}", self.time, self.type_token(), self.object, object_to_string(&self.payload))
    }

    /// SHA-256 of the canonical line.
    pub fn digest(&self) -> [u8; 32] {
        sha256(self.to_line().as_bytes())
    }

    /// Parse a line and require it to be canonical.
    pub fn parse(line: &str) -> Result<Entry, KernelError> {
        let mut it = line.splitn(4, ' ');
        let (t, ty, obj, pl) = match (it.next(), it.next(), it.next(), it.next()) {
            (Some(a), Some(b), Some(c), Some(d)) => (a, b, c, d),
            _ => return Err(KernelError::Fields),
        };
        let time = Time::parse(t)?;
        let (profile, ty) = parse_type(ty)?;
        if !object_ok(obj) {
            return Err(KernelError::Object);
        }
        let payload = parse_object(pl).map_err(|e| KernelError::Payload(e.0.to_string()))?;
        let e = Entry { time, profile, ty, object: obj.to_string(), payload };
        if e.to_line() != line {
            return Err(KernelError::NotCanonical);
        }
        Ok(e)
    }

    /// The kernel's structural rules: every entry has a recorder, and the
    /// eight bootstrap verbs carry the keys the kernel needs.
    pub fn check(&self) -> Result<(), KernelError> {
        need_str(self, "rec")?;
        match self.verb() {
            Verb::CREATE => {
                need_str(self, "kind")?;
            }
            Verb::BIND => {
                if self.get("pack").is_none() && self.get("to").is_none() {
                    return Err(KernelError::Missing("pack or to"));
                }
                if let Some(p) = self.get("pack") {
                    let ok = p.as_str().and_then(|s| s.strip_prefix("sha256:")).map(is_hex64).unwrap_or(false);
                    if !ok {
                        return Err(KernelError::Invalid("pack must be sha256:<hex>".to_string()));
                    }
                }
            }
            Verb::GRANT | Verb::REVOKE => {
                need_str(self, "to")?;
                need_str(self, "may")?;
            }
            Verb::AMEND => {
                let r = need_str(self, "ref")?;
                if Address::parse(r).is_none() {
                    return Err(KernelError::Invalid("ref must be an address".to_string()));
                }
                need_str(self, "reason")?;
            }
            Verb::OPEN => match need_str(self, "kind")? {
                "genesis" => {
                    need_str(self, "log")?;
                    need_str(self, "clock")?;
                    need_str(self, "tz")?;
                    let roots = self.get("root").and_then(Value::as_arr).ok_or(KernelError::Missing("root"))?;
                    if roots.is_empty() || roots.iter().any(|r| r.as_str().is_none()) {
                        return Err(KernelError::Invalid("root must be a non-empty list of actors".to_string()));
                    }
                    if roots[0].as_str() != self.rec() {
                        return Err(KernelError::Invalid("genesis is recorded by its first root".to_string()));
                    }
                }
                "segment" => {
                    if !need_str(self, "prev").map(is_hex64)? {
                        return Err(KernelError::Invalid("prev must be 64 hex".to_string()));
                    }
                }
                k => return Err(KernelError::Invalid(alloc::format!("unknown OPEN kind {k:?}"))),
            },
            Verb::COMMIT => match need_str(self, "kind")? {
                "seal" => {
                    self.get("n").and_then(Value::as_int).ok_or(KernelError::Missing("n"))?;
                    for k in ["sha256", "chain"] {
                        if !is_hex64(need_str(self, k)?) {
                            return Err(KernelError::Invalid(alloc::format!("{k} must be 64 hex")));
                        }
                    }
                }
                "close" => {
                    need_str(self, "book")?;
                    if Address::parse(need_str(self, "upto")?).is_none() {
                        return Err(KernelError::Invalid("upto must be an address".to_string()));
                    }
                    if !is_hex64(need_str(self, "balances")?) {
                        return Err(KernelError::Invalid("balances must be 64 hex".to_string()));
                    }
                }
                k => return Err(KernelError::Invalid(alloc::format!("unknown COMMIT kind {k:?}"))),
            },
            Verb::ACK
                if !is_hex64(need_str(self, "digest")?) => {
                    return Err(KernelError::Invalid("digest must be 64 hex".to_string()));
                }
            _ => {}
        }
        Ok(())
    }
}

fn need_str<'a>(e: &'a Entry, k: &'static str) -> Result<&'a str, KernelError> {
    e.get_str(k).ok_or(KernelError::Missing(k))
}

fn parse_type(tok: &str) -> Result<(Option<char>, Type), KernelError> {
    let bad = || KernelError::Type(tok.to_string());
    let (profile, rest) = match tok.split_once(':') {
        Some((p, r)) => {
            let mut cs = p.chars();
            match (cs.next(), cs.next()) {
                (Some(c), None) if c.is_ascii_uppercase() => (Some(c), r),
                _ => return Err(bad()),
            }
        }
        None => (None, tok),
    };
    let (name, phase) = match rest.as_bytes().last() {
        Some(b'(') => (&rest[..rest.len() - 1], Phase::Begin),
        Some(b')') => (&rest[..rest.len() - 1], Phase::End),
        Some(b'*') => (&rest[..rest.len() - 1], Phase::Sample),
        _ => (rest, Phase::Instant),
    };
    let verb = Verb::from_name(name).ok_or_else(bad)?;
    Ok((profile, Type::new(phase, verb)))
}

/// Check that a value is a decimal string, optionally marked `~`.
pub fn decimal_str(v: &Value) -> Option<(Decimal, bool)> {
    v.as_str().and_then(|s| Decimal::parse_marked(s).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_canonical() {
        let l = r#"1752300000.25 B:ATTEND( lathe-01 {"rec":"human:op-03","run":"p1"}"#;
        let e = Entry::parse(l).unwrap();
        assert_eq!(e.profile, Some('B'));
        assert_eq!(e.ty.byte(), 0x76);
        assert_eq!(e.time.micros, 250_000);
        assert_eq!(e.to_line(), l);
        e.check().unwrap();

        for bad in [
            r#"1752300000.250 ATTEND( x {"rec":"a"}"#,      // trailing zero in time
            r#"1752300000 ATTEND( x {"run":"p1","rec":"a"}"#, // unsorted keys
            r#"1752300000 ATTEND(  x {"rec":"a"}"#,          // double space
            r#"1752300000 attend( x {"rec":"a"}"#,           // unknown verb
            r#"1752300000 BB:ATTEND x {"rec":"a"}"#,         // two-letter profile
            r#"01752300000 NOTE x {"rec":"a"}"#,             // leading zero
        ] {
            assert!(Entry::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn bootstrap_rules() {
        let no_rec = Entry::parse(r#"1 NOTE x {"text":"hi"}"#).unwrap();
        assert_eq!(no_rec.check(), Err(KernelError::Missing("rec")));
        let amend = Entry::parse(r#"1 AMEND x {"reason":"r","rec":"a","ref":"1:0"}"#).unwrap();
        assert!(amend.check().is_err(), "address seq starts at 1");
        let g = Entry::parse(r#"1 OPEN log:x {"clock":"ntp","kind":"genesis","log":"x","rec":"actor:b","root":["actor:a"],"tz":"UTC"}"#).unwrap();
        assert!(g.check().is_err(), "genesis recorded by first root");
    }
}
