//! Packs: bundles of declarations, stored by hash, adopted with one BIND.
//!
//! ```text
//! PACK/1 name=si-units@1
//! 0 CREATE unit:J {"dimension":"energy","kind":"unit","rec":"pack:si-units@1"}
//! ```
//! Every line after the header is a canonical CREATE entry whose recorder is
//! `pack:<name>`. The pack's identity is the SHA-256 of its bytes.

use std::collections::BTreeMap;
use stream_kernel::{sha256_hex, Entry, Verb};

#[derive(Debug, Clone)]
pub struct Pack {
    pub name: String,
    pub hash: String,
    pub decls: Vec<Entry>,
}

#[derive(Debug, Clone)]
pub struct PackError(pub String);

impl std::fmt::Display for PackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "pack: {}", self.0)
    }
}

impl Pack {
    pub fn parse(bytes: &[u8]) -> Result<Pack, PackError> {
        let text = std::str::from_utf8(bytes).map_err(|_| PackError("not UTF-8".into()))?;
        let mut lines = text.lines();
        let name = lines
            .next()
            .and_then(|l| l.strip_prefix("PACK/1 name="))
            .filter(|n| !n.is_empty() && !n.contains(' '))
            .ok_or_else(|| PackError("first line must be PACK/1 name=<name>".into()))?
            .to_string();
        let rec = format!("pack:{name}");
        let mut decls = Vec::new();
        let mut seen: BTreeMap<(String, String), String> = BTreeMap::new();
        for (i, l) in lines.enumerate() {
            let e = Entry::parse(l).map_err(|err| PackError(format!("line {}: {err}", i + 2)))?;
            e.check().map_err(|err| PackError(format!("line {}: {err}", i + 2)))?;
            if e.verb() != Verb::CREATE {
                return Err(PackError(format!("line {}: packs hold CREATE entries only", i + 2)));
            }
            if e.rec() != Some(rec.as_str()) {
                return Err(PackError(format!("line {}: rec must be {rec}", i + 2)));
            }
            let key = (e.get_str("kind").unwrap_or("").to_string(), e.object.clone());
            let line = e.to_line();
            if let Some(prev) = seen.insert(key.clone(), line.clone()) {
                if prev != line {
                    return Err(PackError(format!("line {}: {} {} declared twice", i + 2, key.0, key.1)));
                }
            }
            decls.push(e);
        }
        Ok(Pack { name, hash: sha256_hex(bytes), decls })
    }
}
