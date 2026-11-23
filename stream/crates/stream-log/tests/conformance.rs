//! Run every case in spec/conformance against this implementation.

use std::path::Path;
use stream_log::Log;

#[test]
fn conformance_logs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/conformance");
    let expected = std::fs::read_to_string(root.join("EXPECTED")).unwrap();
    let mut n = 0;
    for line in expected.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let (case, want) = line.split_once(char::is_whitespace).unwrap();
        let want = want.trim();
        let got = match Log::open(&root.join(case)) {
            Err(e) => format!("open: {e}"),
            Ok(log) => match log.verify().first() {
                Some(p) => format!("verify: {p}"),
                None => "ok".to_string(),
            },
        };
        let (kind, text) = want.split_once(": ").unwrap_or((want, ""));
        assert!(got.starts_with(kind) && got.contains(text), "{case}: expected {want:?}, got {got:?}");
        n += 1;
    }
    assert_eq!(n, 6);
}
