//! The reconstruction-after-failure scenario, against the real log.

use std::fs;
use stream_kernel::json::{object_to_string, Object};
use stream_kernel::{kernel_hash, Address, Entry, Time, Value};
use stream_log::{Genesis, Log, Receipt};

const T0: u64 = 1_752_300_000;

fn t(s: u64) -> Time {
    Time::secs(T0 + s)
}

/// Build an entry from a type token and key/value pairs.
fn ent(s: u64, ty: &str, obj: &str, kv: &[(&str, Value)]) -> Entry {
    let mut o = Object::new();
    for (k, v) in kv {
        o.insert(k.to_string(), v.clone());
    }
    Entry::parse(&format!("{} {ty} {obj} {}", T0 + s, object_to_string(&o))).expect("test entry")
}

fn s(v: &str) -> Value {
    Value::str(v)
}
fn i(v: i64) -> Value {
    Value::Int(v)
}
fn legs(l: &[(&str, &str, &str)]) -> Value {
    Value::Arr(
        l.iter()
            .map(|(a, q, u)| {
                let mut o = Object::new();
                o.insert("a".into(), s(a));
                o.insert("q".into(), s(q));
                o.insert("u".into(), s(u));
                Value::Obj(o)
            })
            .collect(),
    )
}

fn new_log() -> (tempfile::TempDir, Log) {
    let d = tempfile::tempdir().unwrap();
    let log = Log::init(
        d.path(),
        Genesis { log: "acme", roots: &["actor:founder"], clock: "ntp", tz: "Europe/London", time: t(0) },
    )
    .unwrap();
    (d, log)
}

#[test]
fn preamble_and_genesis() {
    let (d, log) = new_log();
    let first = fs::read_to_string(d.path().join("seg-000001.log")).unwrap();
    assert!(first.starts_with(&format!("STREAM/1 kernel=sha256:{}\n", kernel_hash())));
    assert_eq!(log.id(), "acme");
    assert_eq!(log.roots(), &["actor:founder".to_string()]);
    assert_eq!(log.last_address(), Address { seg: 1, seq: 1 });
}

#[test]
fn authority_retries_and_gaps() {
    let (_d, mut log) = new_log();
    let root = "actor:founder";
    let g = ent(1, "GRANT", "host:build-01", &[("for", s("agent:*")), ("may", s("record")), ("to", s("host:build-01"))]);
    assert!(log.submit(root, g, t(1)).unwrap().is_accepted());

    // The host records for its agent; actor and recorder stay distinct.
    let r = log.submit("host:build-01", ent(2, "ACT(", "run-1", &[("actor", s("agent:claude-07")), ("ps", i(1)), ("run", s("r1"))]), t(2)).unwrap();
    let a = r.addr().unwrap();
    assert!(r.is_accepted());
    assert_eq!(log.get(a).unwrap().actor(), Some("agent:claude-07"));
    assert_eq!(log.get(a).unwrap().rec(), Some("host:build-01"));

    // One operator cannot punch for another; the refusal is journaled.
    let forged = ent(3, "B:ATTEND(", "lathe-01", &[("actor", s("human:op-03")), ("ps", i(1))]);
    let r1 = log.submit("human:op-07", forged.clone(), t(3)).unwrap();
    let r2 = log.submit("human:op-07", forged, t(4)).unwrap();
    assert!(matches!(&r1, Receipt::Rejected { at: Some(_), reason } if reason.contains("may not record")));
    assert_eq!(r1, r2, "a retry gets the original answer");

    // Lost acknowledgement: same entry, same ps, same address.
    let punch = ent(5, "B:ATTEND(", "lathe-01", &[("ps", i(1)), ("run", s("p1"))]);
    let a1 = log.submit("human:op-03", punch.clone(), t(5)).unwrap();
    let a2 = log.submit("human:op-03", punch, t(9)).unwrap();
    assert_eq!(a2, Receipt::Duplicate(a1.addr().unwrap()));

    // Same ps, different claim: refused, nothing written.
    let before = log.entries().len();
    let c = log.submit("human:op-03", ent(6, "B:ATTEND(", "lathe-02", &[("ps", i(1))]), t(6)).unwrap();
    assert!(matches!(c, Receipt::Rejected { at: None, .. }));
    assert_eq!(log.entries().len(), before);

    // Two identical readings that really happened twice: two entries.
    for ps in [1, 2] {
        let m = ent(7, "MEASURE", "bin-7", &[("ps", i(ps)), ("q", s("92")), ("u", s("kg"))]);
        assert!(log.submit("scale:s1", m, t(7)).unwrap().is_accepted());
    }

    // Gaps: op-03 sent ps 1 then 3. op-07's ps 1 was refused, so it counts as received.
    log.submit("human:op-03", ent(8, "B:ATTEND)", "lathe-01", &[("ps", i(3)), ("run", s("p1"))]), t(8)).unwrap();
    let gaps = log.gaps();
    assert_eq!(gaps.get("human:op-03"), Some(&vec![2]));
    assert!(!gaps.contains_key("human:op-07"));
}

#[test]
fn postings_and_amendments() {
    let (_d, mut log) = new_log();
    let acct = "human:acct-1";
    let post = |s_: u64, ps: i64, l: &[(&str, &str, &str)]| ent(s_, "PAY", "run-r1", &[("legs", legs(l)), ("ps", i(ps))]);

    let ok = log.submit(acct, post(1, 1, &[("inv:raw", "-100", "kg"), ("inv:product", "92", "kg"), ("inv:scrap", "8", "kg")]), t(1)).unwrap();
    let orig = ok.addr().unwrap();
    assert!(ok.is_accepted());

    // Two postings wrong by +5 and -5 in one upload: each refused on its own.
    let r1 = log.submit(acct, post(2, 2, &[("a", "10", "USD"), ("b", "-5", "USD")]), t(2)).unwrap();
    let r2 = log.submit(acct, post(3, 3, &[("a", "-5", "USD"), ("b", "10", "USD")]), t(3)).unwrap();
    assert!(matches!(r1, Receipt::Rejected { ref reason, .. } if reason.contains("unbalanced")));
    assert!(matches!(r2, Receipt::Rejected { .. }));

    // Balanced in USD but not in J: refused.
    let mixed = post(4, 4, &[("e:host", "-52.4", "J"), ("e:run", "52.0", "J"), ("cost", "0.019", "USD"), ("ap", "-0.019", "USD")]);
    assert!(matches!(log.submit(acct, mixed, t(4)).unwrap(), Receipt::Rejected { ref reason, .. } if reason.contains("-/J")));

    // Amend, never erase; the second amendment of the original is refused.
    let am = |s_: u64, ps: i64, r: Address, p: &str, sc: &str| {
        ent(s_, "AMEND", "run-r1", &[
            ("legs", legs(&[("inv:raw", "-100", "kg"), ("inv:product", p, "kg"), ("inv:scrap", sc, "kg")])),
            ("ps", i(ps)), ("reason", s("weighbridge recheck")), ("ref", s(&r.to_string())),
        ])
    };
    let a1 = log.submit(acct, am(5, 5, orig, "91", "9"), t(5)).unwrap();
    assert!(a1.is_accepted());
    assert!(log.is_superseded(orig));
    let again = log.submit(acct, am(6, 6, orig, "90", "10"), t(6)).unwrap();
    assert!(matches!(again, Receipt::Rejected { ref reason, .. } if reason.contains("already superseded")));
    let chain = log.submit(acct, am(7, 7, a1.addr().unwrap(), "90", "10"), t(7)).unwrap();
    assert!(chain.is_accepted(), "the latest version can be amended");
    assert_eq!(log.get(orig).unwrap().get("legs"), Some(&legs(&[("inv:raw", "-100", "kg"), ("inv:product", "92", "kg"), ("inv:scrap", "8", "kg")])));
}

#[test]
fn packs_register_and_conflicts() {
    let (_d, mut log) = new_log();
    let root = "actor:founder";
    let pack = b"PACK/1 name=si-units@1\n\
0 CREATE unit:J {\"conserved\":true,\"dimension\":\"energy\",\"kind\":\"unit\",\"rec\":\"pack:si-units@1\"}\n\
0 CREATE unit:kg {\"conserved\":true,\"dimension\":\"mass\",\"kind\":\"unit\",\"rec\":\"pack:si-units@1\"}\n";
    assert!(log.adopt(root, pack, Object::new(), t(1)).unwrap().is_accepted());
    assert_eq!(log.register().get("unit", "unit:J").unwrap().pack.as_deref(), Some("si-units@1"));
    assert_eq!(log.register().of_kind("unit").count(), 2);

    // A pack that redefines J differently is refused.
    let bad = b"PACK/1 name=odd@1\n0 CREATE unit:J {\"dimension\":\"mass\",\"kind\":\"unit\",\"rec\":\"pack:odd@1\"}\n";
    assert!(matches!(log.adopt(root, bad, Object::new(), t(2)).unwrap(), Receipt::Rejected { ref reason, .. } if reason.contains("redefines")));

    // Only an admin may declare.
    let c = ent(3, "CREATE", "machine:lathe-01", &[("kind", s("machine")), ("name", s("Lathe 1"))]);
    assert!(matches!(log.submit("human:op-03", c.clone(), t(3)).unwrap(), Receipt::Rejected { .. }));
    let ca = log.submit(root, c, t(3)).unwrap();
    assert!(ca.is_accepted());

    // Renaming is one AMEND of the declaration.
    let mut set = Object::new();
    set.insert("name".into(), s("Lathe One"));
    let ren = ent(4, "AMEND", "machine:lathe-01", &[("reason", s("rename")), ("ref", s(&ca.addr().unwrap().to_string())), ("set", Value::Obj(set))]);
    assert!(log.submit(root, ren, t(4)).unwrap().is_accepted());
    assert_eq!(log.register().get("machine", "machine:lathe-01").unwrap().payload.get("name"), Some(&s("Lathe One")));

    // The journal's own entries cannot be submitted.
    let fake = ent(5, "OPEN", "seg-9", &[("kind", s("segment")), ("prev", s(&"0".repeat(64)))]);
    assert!(matches!(log.submit(root, fake, t(5)).unwrap(), Receipt::Rejected { .. }));
}

#[test]
fn seals_rebuild_and_tamper() {
    let (d, mut log) = new_log();
    for k in 1..=3 {
        log.submit("meter:pdu-4", ent(k, "MEASURE*", "pdu-4.out3", &[("ps", i(k as i64)), ("q", s("38")), ("u", s("J"))]), t(k)).unwrap();
    }
    log.seal(t(10)).unwrap();
    log.submit("meter:pdu-4", ent(11, "MEASURE*", "pdu-4.out3", &[("ps", i(4)), ("q", s("41")), ("u", s("J"))]), t(11)).unwrap();
    assert_eq!(log.last_address().seg, 2, "writing after a seal opens a new segment");
    assert!(log.verify().is_empty(), "{:?}", log.verify());

    // Rebuild from the files alone: same entries, same retry index.
    let reopened = Log::open(d.path()).unwrap();
    assert_eq!(reopened.entries(), log.entries());
    let mut reopened = reopened;
    let dup = reopened.submit("meter:pdu-4", ent(1, "MEASURE*", "pdu-4.out3", &[("ps", i(1)), ("q", s("38")), ("u", s("J"))]), t(20)).unwrap();
    assert_eq!(dup, Receipt::Duplicate(Address { seg: 1, seq: 2 }));

    // Change one byte in the sealed segment.
    let p = d.path().join("seg-000001.log");
    let text = fs::read_to_string(&p).unwrap().replacen("\"q\":\"38\"", "\"q\":\"39\"", 1);
    fs::write(&p, text).unwrap();
    let problems = Log::open(d.path()).unwrap().verify();
    assert!(problems.iter().any(|m| m.contains("seg 1: body hash mismatch")), "{problems:?}");
}
