use stream_books::{attribution, balances, close_entry, spans, verify_closes};
use stream_kernel::json::{object_to_string, Object};
use stream_kernel::{Decimal, Entry, Time, Value};
use stream_log::{Genesis, Log};

const T0: u64 = 1_752_300_000;
const CONSERVED: &[&str] = &["xs", "J", "B", "ti", "to", "#", "kg", "USD"];

fn ent(t: &str, ty: &str, obj: &str, kv: &[(&str, Value)]) -> Entry {
    let mut o = Object::new();
    for (k, v) in kv {
        o.insert(k.to_string(), v.clone());
    }
    Entry::parse(&format!("{t} {ty} {obj} {}", object_to_string(&o))).unwrap()
}
fn s(v: &str) -> Value {
    Value::str(v)
}
fn uses(kv: &[(&str, &str)]) -> Value {
    let mut o = Object::new();
    for (k, v) in kv {
        o.insert(k.to_string(), s(v));
    }
    Value::Obj(o)
}
fn d(x: &str) -> Decimal {
    Decimal::parse(x).unwrap()
}

fn log() -> (tempfile::TempDir, Log) {
    let dir = tempfile::tempdir().unwrap();
    let mut l = Log::init(dir.path(), Genesis { log: "t", roots: &["actor:root"], clock: "ntp", tz: "UTC", time: Time::secs(T0) }).unwrap();
    let g = ent(&T0.to_string(), "GRANT", "host:build-01", &[("for", s("agent:*")), ("may", s("record")), ("to", s("host:build-01"))]);
    assert!(l.submit("actor:root", g, Time::secs(T0)).unwrap().is_accepted());
    (dir, l)
}

/// The agent task from the trial: host totals at each level, depth = process.
fn cascade(l: &mut Log) {
    let a = ("actor", s("agent:claude-07"));
    let rows: Vec<Entry> = vec![
        ent("1752300000", "ACT(", "task-42", &[a.clone(), ("run", s("k42"))]),
        ent("1752300000.1", "QUERY(", "task-42", &[a.clone(), ("parent", s("k42")), ("run", s("k42.m1"))]),
        ent("1752300004.3", "QUERY)", "task-42", &[a.clone(), ("run", s("k42.m1")), ("use", uses(&[("ti", "12000"), ("to", "900")]))]),
        ent("1752300004.5", "ACT(", "task-42", &[a.clone(), ("parent", s("k42")), ("run", s("k42.t1"))]),
        ent("1752300004.6", "ACT(", "task-42", &[a.clone(), ("parent", s("k42.t1")), ("run", s("k42.t1.p1"))]),
        ent("1752300039", "ACT)", "task-42", &[a.clone(), ("run", s("k42.t1.p1")), ("use", uses(&[("B", "51000000"), ("J", "~470"), ("xs", "32.0")]))]),
        ent("1752300040", "ACT)", "task-42", &[a.clone(), ("run", s("k42.t1")), ("use", uses(&[("#", "412"), ("B", "52000000"), ("J", "~480"), ("xs", "33.1")]))]),
        ent("1752300041", "QUERY(", "task-42", &[a.clone(), ("parent", s("k42")), ("run", s("k42.m2"))]),
        ent("1752300044", "QUERY)", "task-42", &[a.clone(), ("run", s("k42.m2")), ("use", uses(&[("ti", "6000"), ("to", "1500")]))]),
        ent("1752300045", "ACT(", "task-42", &[a.clone(), ("parent", s("k42")), ("run", s("k42.t2"))]),
        ent("1752300045", "ACT(", "task-42", &[a.clone(), ("parent", s("k42")), ("run", s("k42.t3"))]),
        ent("1752300053", "ACT)", "task-42", &[a.clone(), ("run", s("k42.t2")), ("use", uses(&[("J", "~40"), ("xs", "3.0")]))]),
        ent("1752300055", "ACT)", "task-42", &[a.clone(), ("run", s("k42.t3")), ("status", s("fail")), ("use", uses(&[("J", "~35"), ("xs", "2.4")]))]),
        ent("1752300060", "ACT)", "task-42", &[a, ("pool", s("host:build-01")), ("run", s("k42")),
            ("use", uses(&[("J", "610"), ("ti", "18000"), ("to", "2400"), ("xs", "41.0")]))]),
    ];
    for (i, e) in rows.into_iter().enumerate() {
        let mut e = e;
        e.payload.insert("ps".into(), Value::Int(i as i64 + 1));
        let r = l.submit("host:build-01", e, Time::secs(T0 + 61)).unwrap();
        assert!(r.is_accepted(), "{r:?}");
    }
}

#[test]
fn remainder_is_work_below_recorded_depth() {
    let (_d, mut l) = log();
    cascade(&mut l);
    let a = attribution(&l, CONSERVED);
    let top = &a["k42"];
    assert_eq!(top.wall, d("60"));
    assert_eq!(top.remainder["xs"], (d("2.5"), false));
    assert_eq!(top.remainder["J"], (d("55"), true), "estimates propagate");
    assert_eq!(top.remainder["ti"], (d("0"), false));
    assert_eq!(top.utilisation, Some(0.683));
    let t1 = &a["k42.t1"];
    assert_eq!(t1.wall, d("35.5"));
    assert_eq!(t1.remainder["#"], (d("412"), false));
    assert_eq!(t1.parallelism, Some(0.969));
    assert_eq!(a["k42.m1"].wall, d("4.2"));
}

#[test]
fn implied_postings_from_use_and_pool() {
    let (_d, mut l) = log();
    cascade(&mut l);
    let b = balances(&l, None);
    let get = |acct: &str, unit: &str| b.get(&("-".into(), acct.into(), unit.into())).map(|x| x.q);
    assert_eq!(get("host:build-01", "J"), Some(d("-610")));
    assert_eq!(get("task-42", "J"), Some(d("610")));
    assert_eq!(get("task-42", "ti"), Some(d("18000")));
    // Only the top span named a pool, so only its totals move: no double counting.
    assert_eq!(get("task-42", "xs"), Some(d("41")));
}

#[test]
fn explicit_postings_amend_and_close() {
    let (_d, mut l) = log();
    let leg = |a: &str, q: &str| {
        let mut o = Object::new();
        o.insert("a".into(), s(a));
        o.insert("q".into(), s(q));
        o.insert("u".into(), s("kg"));
        Value::Obj(o)
    };
    let post = ent("1752300100", "PAY", "run-r1", &[("legs", Value::Arr(vec![leg("inv:raw", "-100"), leg("inv:product", "92"), leg("inv:scrap", "8")]))]);
    let orig = l.submit("actor:root", post, Time::secs(T0 + 100)).unwrap().addr().unwrap();
    let before = l.last_address();

    let close = close_entry(&l, "-", before, Time::secs(T0 + 150));
    assert!(l.submit("actor:root", close, Time::secs(T0 + 150)).unwrap().is_accepted());

    let am = ent("1752300200", "AMEND", "run-r1", &[
        ("legs", Value::Arr(vec![leg("inv:raw", "-100"), leg("inv:product", "91"), leg("inv:scrap", "9")])),
        ("reason", s("recheck")), ("ref", s(&orig.to_string())),
    ]);
    assert!(l.submit("actor:root", am, Time::secs(T0 + 200)).unwrap().is_accepted());

    let now = balances(&l, None);
    assert_eq!(now[&("-".into(), "inv:product".into(), "kg".into())].q, d("91"));
    // The close still verifies: the amendment came after it and does not rewrite the past.
    let then = balances(&l, Some(before));
    assert_eq!(then[&("-".into(), "inv:product".into(), "kg".into())].q, d("92"));
    assert!(verify_closes(&l).is_empty());
}

#[test]
fn overlapping_punches_stay_separate() {
    let (_d, mut l) = log();
    let rows = [
        ("human:op-03", "1752300000", "B:ATTEND(", 1),
        ("human:op-07", "1752300600", "B:ATTEND(", 1),
        ("human:op-03", "1752301200", "B:ATTEND)", 2),
        ("human:op-07", "1752303000", "B:ATTEND)", 2),
        ("human:op-09", "1752300000", "B:ATTEND(", 1),
    ];
    for (who, t, ty, ps) in rows {
        let e = ent(t, ty, "lathe-01", &[("ps", Value::Int(ps))]);
        assert!(l.submit(who, e, Time::secs(T0 + 4000)).unwrap().is_accepted());
    }
    let (sp, anomalies) = spans(&l);
    let mut closed: Vec<(String, Decimal)> = sp.iter().filter_map(|s| s.wall().map(|w| (s.actor.clone(), w))).collect();
    closed.sort();
    assert_eq!(closed, vec![("human:op-03".into(), d("1200")), ("human:op-07".into(), d("2400"))]);
    assert!(sp.iter().any(|s| s.actor == "human:op-09" && s.end.is_none()), "op-09 stays open, not invented");
    assert!(anomalies.is_empty());
}
