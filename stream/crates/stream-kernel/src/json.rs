//! Canonical JSON: the only payload form the kernel accepts.
//!
//! Sorted keys, no whitespace, integers only, minimal string escaping.
//! Parsing is strict; an entry is canonical iff `serialize(parse(x)) == x`.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

pub type Object = BTreeMap<String, Value>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    Arr(Vec<Value>),
    Obj(Object),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonError(pub &'static str);

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }
    pub fn as_obj(&self) -> Option<&Object> {
        match self {
            Value::Obj(o) => Some(o),
            _ => None,
        }
    }
    pub fn as_arr(&self) -> Option<&[Value]> {
        match self {
            Value::Arr(a) => Some(a),
            _ => None,
        }
    }
    pub fn str(s: &str) -> Value {
        Value::Str(s.to_string())
    }
}

// ---------------------------------------------------------------- serialize

pub fn write_value(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(i) => out.push_str(&i.to_string()),
        Value::Str(s) => write_str(s, out),
        Value::Arr(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(x, out);
            }
            out.push(']');
        }
        Value::Obj(o) => write_object(o, out),
    }
}

pub fn write_object(o: &Object, out: &mut String) {
    out.push('{');
    for (i, (k, v)) in o.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write_str(k, out);
        out.push(':');
        write_value(v, out);
    }
    out.push('}');
}

pub fn object_to_string(o: &Object) -> String {
    let mut s = String::new();
    write_object(o, &mut s);
    s
}

fn write_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{0C}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => {
                out.push_str(&alloc::format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

// -------------------------------------------------------------------- parse

pub fn parse_object(s: &str) -> Result<Object, JsonError> {
    let mut p = Parser { b: s.as_bytes(), i: 0 };
    let v = p.value(0)?;
    if p.i != p.b.len() {
        return Err(JsonError("trailing bytes"));
    }
    match v {
        Value::Obj(o) => Ok(o),
        _ => Err(JsonError("payload is not an object")),
    }
}

/// Parse one JSON value from the start of `s`; returns it and the bytes used.
pub fn parse_value_prefix(s: &str) -> Result<(Value, usize), JsonError> {
    let mut p = Parser { b: s.as_bytes(), i: 0 };
    let v = p.value(0)?;
    Ok((v, p.i))
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

const MAX_DEPTH: usize = 32;

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }
    fn eat(&mut self, c: u8) -> Result<(), JsonError> {
        if self.peek() == Some(c) {
            self.i += 1;
            Ok(())
        } else {
            Err(JsonError("unexpected character"))
        }
    }
    fn lit(&mut self, w: &str, v: Value) -> Result<Value, JsonError> {
        if self.b[self.i..].starts_with(w.as_bytes()) {
            self.i += w.len();
            Ok(v)
        } else {
            Err(JsonError("bad literal"))
        }
    }
    fn value(&mut self, depth: usize) -> Result<Value, JsonError> {
        if depth > MAX_DEPTH {
            return Err(JsonError("nested too deeply"));
        }
        match self.peek() {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => Ok(Value::Str(self.string()?)),
            Some(b't') => self.lit("true", Value::Bool(true)),
            Some(b'f') => self.lit("false", Value::Bool(false)),
            Some(b'n') => self.lit("null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.int(),
            _ => Err(JsonError("expected a value")),
        }
    }
    fn object(&mut self, depth: usize) -> Result<Value, JsonError> {
        self.eat(b'{')?;
        let mut o = Object::new();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(Value::Obj(o));
        }
        loop {
            let k = self.string()?;
            self.eat(b':')?;
            let v = self.value(depth + 1)?;
            if o.insert(k, v).is_some() {
                return Err(JsonError("duplicate key"));
            }
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Value::Obj(o));
                }
                _ => return Err(JsonError("expected , or }")),
            }
        }
    }
    fn array(&mut self, depth: usize) -> Result<Value, JsonError> {
        self.eat(b'[')?;
        let mut a = Vec::new();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Ok(Value::Arr(a));
        }
        loop {
            a.push(self.value(depth + 1)?);
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(Value::Arr(a));
                }
                _ => return Err(JsonError("expected , or ]")),
            }
        }
    }
    fn int(&mut self) -> Result<Value, JsonError> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        let ds = self.i;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.i += 1;
        }
        if self.i == ds {
            return Err(JsonError("bad number"));
        }
        if matches!(self.peek(), Some(b'.' | b'e' | b'E')) {
            return Err(JsonError("numbers are integers; quantities are strings"));
        }
        let txt = core::str::from_utf8(&self.b[start..self.i]).map_err(|_| JsonError("utf8"))?;
        txt.parse::<i64>().map(Value::Int).map_err(|_| JsonError("integer out of range"))
    }
    fn hex4(&mut self) -> Result<u32, JsonError> {
        let s = self.b.get(self.i..self.i + 4).ok_or(JsonError("short \\u escape"))?;
        let s = core::str::from_utf8(s).map_err(|_| JsonError("utf8"))?;
        self.i += 4;
        u32::from_str_radix(s, 16).map_err(|_| JsonError("bad \\u escape"))
    }
    fn string(&mut self) -> Result<String, JsonError> {
        self.eat(b'"')?;
        let mut out = String::new();
        loop {
            let start = self.i;
            while let Some(c) = self.peek() {
                if c == b'"' || c == b'\\' || c < 0x20 {
                    break;
                }
                self.i += 1;
            }
            out.push_str(core::str::from_utf8(&self.b[start..self.i]).map_err(|_| JsonError("utf8"))?);
            match self.peek() {
                Some(b'"') => {
                    self.i += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.i += 1;
                    let e = self.peek().ok_or(JsonError("bad escape"))?;
                    self.i += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{08}'),
                        b'f' => out.push('\u{0C}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let mut cp = self.hex4()?;
                            if (0xD800..0xDC00).contains(&cp) {
                                if self.b.get(self.i..self.i + 2) != Some(b"\\u") {
                                    return Err(JsonError("lone surrogate"));
                                }
                                self.i += 2;
                                let lo = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&lo) {
                                    return Err(JsonError("lone surrogate"));
                                }
                                cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                            }
                            out.push(char::from_u32(cp).ok_or(JsonError("bad code point"))?);
                        }
                        _ => return Err(JsonError("bad escape")),
                    }
                }
                _ => return Err(JsonError("unterminated string or raw control character")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canon(s: &str) -> bool {
        parse_object(s).map(|o| object_to_string(&o) == s).unwrap_or(false)
    }

    #[test]
    fn canonical_forms() {
        assert!(canon(r#"{"a":1,"b":[true,null,"x"],"c":{"d":"q\"\\\n"}}"#));
        assert!(canon(r#"{}"#));
        assert!(canon("{\"é\":\"日本\"}"));
        assert!(!canon(r#"{"b":1,"a":2}"#), "unsorted");
        assert!(!canon(r#"{"a": 1}"#), "whitespace");
        assert!(!canon(r#"{"a":"\/"}"#), "needless escape");
        let esc = alloc::format!("{{\"a\":\"{}u0041\"}}", '\\');
        assert!(!canon(&esc), "needless unicode escape");
        assert!(!canon(r#"{"a":01}"#), "leading zero");
        assert!(!canon(r#"{"a":-0}"#), "negative zero");
        assert!(parse_object(r#"{"a":1.5}"#).is_err(), "float");
        assert!(parse_object(r#"{"a":1,"a":2}"#).is_err(), "duplicate");
        assert!(parse_object(r#"[1]"#).is_err(), "not an object");
    }
}
