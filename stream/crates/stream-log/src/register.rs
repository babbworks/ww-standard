//! The Register: everything the log has declared, folded from CREATE
//! entries, adopted packs and AMENDs of declarations.

use std::collections::BTreeMap;
use stream_kernel::json::Object;
use stream_kernel::{Address, Entry, Value, Verb};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decl {
    pub kind: String,
    pub id: String,
    /// Where it took effect: the CREATE, the adopting BIND, or the AMEND.
    pub addr: Address,
    /// The declaration's fields, without `rec`.
    pub payload: Object,
    /// The pack it came from, if any.
    pub pack: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Register {
    decls: BTreeMap<(String, String), Decl>,
}

pub(crate) fn strip_rec(p: &Object) -> Object {
    let mut o = p.clone();
    o.remove("rec");
    o
}

impl Register {
    pub(crate) fn declare(&mut self, e: &Entry, addr: Address, pack: Option<&str>) {
        let kind = e.get_str("kind").unwrap_or("").to_string();
        self.decls.insert(
            (kind.clone(), e.object.clone()),
            Decl { kind, id: e.object.clone(), addr, payload: strip_rec(&e.payload), pack: pack.map(String::from) },
        );
    }

    /// An AMEND of a CREATE merges its `set` object into the declaration.
    pub(crate) fn amend(&mut self, e: &Entry, addr: Address, target: Option<&Entry>) {
        let Some(t) = target.filter(|t| t.verb() == Verb::CREATE) else { return };
        let key = (t.get_str("kind").unwrap_or("").to_string(), t.object.clone());
        if let (Some(d), Some(set)) = (self.decls.get_mut(&key), e.get("set").and_then(Value::as_obj)) {
            for (k, v) in set {
                d.payload.insert(k.clone(), v.clone());
            }
            d.addr = addr;
        }
    }

    pub fn get(&self, kind: &str, id: &str) -> Option<&Decl> {
        self.decls.get(&(kind.to_string(), id.to_string()))
    }

    pub fn of_kind<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = &'a Decl> + 'a {
        self.decls.values().filter(move |d| d.kind == kind)
    }

    pub fn all(&self) -> impl Iterator<Item = &Decl> {
        self.decls.values()
    }

    pub fn len(&self) -> usize {
        self.decls.len()
    }
    pub fn is_empty(&self) -> bool {
        self.decls.is_empty()
    }
}
