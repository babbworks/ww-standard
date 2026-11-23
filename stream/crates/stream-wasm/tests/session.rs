//! The browser interface, driven natively.

use stream_wasm::Session;

const T: i64 = 1_752_300_000_000_000;

fn call(s: &mut Session, req: &str) -> String {
    let a = s.handle(req);
    assert!(!a.contains("\"error\""), "{req} -> {a}");
    a
}

fn submit(s: &mut Session, who: &str, line: &str, dt: i64) -> String {
    let req = format!(r#"{{"op":"submit","as":"{who}","line":{},"now_us":{}}}"#, json(line), T + dt);
    call(s, &req)
}

fn json(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn setup() -> Session {
    let mut s = Session::new();
    call(&mut s, &format!(r#"{{"op":"init","log":"acme","root":"human:ops-lead","clock":"browser","tz":"Europe/London","now_us":{T}}}"#));
    for p in ["si-units@1", "agent-ops@1"] {
        let a = call(&mut s, &format!(r#"{{"op":"adopt","as":"human:ops-lead","pack":"{p}","now_us":{T}}}"#));
        assert!(a.contains("accepted"), "{a}");
    }
    let a = submit(&mut s, "human:ops-lead", r#"1752300001 GRANT host:build-01 {"for":"agent:*","may":"record","to":"host:build-01"}"#, 1_000_000);
    assert!(a.contains("accepted"), "{a}");
    s
}

#[test]
fn open_interval_then_close_with_use() {
    let mut s = setup();
    let a = submit(&mut s, "host:build-01", r#"1752300002 R:ACT( task-42 {"actor":"agent:claude-07","ps":1,"run":"k42"}"#, 2_000_000);
    assert!(a.contains("accepted"), "{a}");
    let a = submit(&mut s, "host:build-01", r#"1752300003 R:QUERY( task-42 {"actor":"agent:claude-07","parent":"k42","ps":2,"run":"k42.m1"}"#, 3_000_000);
    assert!(a.contains("accepted"), "{a}");
    let a = submit(&mut s, "host:build-01", r#"1752300004 R:QUERY) task-42 {"actor":"agent:claude-07","pool":"host:build-01","ps":3,"run":"k42.m1","use":{"ti":"6000","to":"400"}}"#, 4_000_000);
    assert!(a.contains("accepted"), "{a}");
    let a = submit(&mut s, "host:build-01", r#"1752300005 MEASURE* meter:pdu-4 {"ps":4,"q":"38","scope":"host:build-01","u":"J"}"#, 5_000_000);
    assert!(a.contains("accepted"), "{a}");

    let now = call(&mut s, r#"{"op":"now"}"#);
    assert!(now.contains(r#""from_children":{"ti":"6000","to":"400"}"#), "{now}");
    assert!(now.contains(r#""from_meters":{"host:build-01":{"J":"38"}}"#), "{now}");
    assert!(now.contains(r#""next_ps":{"host:build-01":5}"#), "{now}");
}

#[test]
fn refusal_is_journaled_and_retry_gets_same_answer() {
    let mut s = setup();
    let line = r#"1752300002 R:ACT( task-9 {"actor":"agent:claude-07","ps":1,"run":"k9"}"#;
    let a = submit(&mut s, "human:op-07", line, 2_000_000);
    assert!(a.contains("rejected") && a.contains("may not record for agent:claude-07"), "{a}");
    let b = submit(&mut s, "human:op-07", line, 9_000_000);
    assert_eq!(a, b);
    let chk = call(&mut s, &format!(r#"{{"op":"check","as":"host:build-01","line":{}}}"#, json(line)));
    assert!(chk.contains(r#""ok":true"#), "{chk}");
    assert!(call(&mut s, r#"{"op":"now"}"#).contains("may not record for agent:claude-07"));
}

#[test]
fn state_round_trips_and_verifies() {
    let mut s = setup();
    submit(&mut s, "host:build-01", r#"1752300002 R:ACT( task-42 {"actor":"agent:claude-07","ps":1,"run":"k42"}"#, 2_000_000);
    call(&mut s, &format!(r#"{{"op":"seal","now_us":{}}}"#, T + 3_000_000));
    submit(&mut s, "host:build-01", r#"1752300004 R:ACT) task-42 {"actor":"agent:claude-07","ps":2,"run":"k42"}"#, 4_000_000);
    let state = call(&mut s, r#"{"op":"state"}"#);
    let before = call(&mut s, r#"{"op":"now"}"#);

    let mut t = Session::new();
    let req = format!(r#"{{"op":"open",{}"#, &state[1..]);
    call(&mut t, &req);
    assert_eq!(call(&mut t, r#"{"op":"now"}"#), before);
    assert_eq!(call(&mut t, r#"{"op":"verify"}"#), r#"{"problems":[]}"#);
}
