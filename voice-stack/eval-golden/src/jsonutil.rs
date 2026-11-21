// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Small JSON writer. Object keys are sorted so a report diff stays stable.

use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub enum J {
    Null,
    Bool(bool),
    Int(i64),
    Num(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(BTreeMap<String, J>),
}

impl J {
    pub fn obj(pairs: Vec<(&str, J)>) -> J {
        let mut map = BTreeMap::new();
        for (k, v) in pairs {
            map.insert(k.to_string(), v);
        }
        J::Obj(map)
    }

    pub fn str(s: impl Into<String>) -> J {
        J::Str(s.into())
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    fn write(&self, out: &mut String) {
        match self {
            J::Null => out.push_str("null"),
            J::Bool(true) => out.push_str("true"),
            J::Bool(false) => out.push_str("false"),
            J::Int(n) => out.push_str(&n.to_string()),
            J::Num(n) => {
                if n.is_finite() {
                    let mut s = format!("{n:.6}");
                    if s.contains('.') {
                        while s.ends_with('0') {
                            s.pop();
                        }
                        if s.ends_with('.') {
                            s.pop();
                        }
                    }
                    if s == "-0" {
                        s = "0".into();
                    }
                    out.push_str(&s);
                } else {
                    out.push_str("null");
                }
            }
            J::Str(s) => push_escaped(out, s),
            J::Arr(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write(out);
                }
                out.push(']');
            }
            J::Obj(map) => {
                out.push('{');
                for (i, (k, v)) in map.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    push_escaped(out, k);
                    out.push(':');
                    v.write(out);
                }
                out.push('}');
            }
        }
    }
}

fn push_escaped(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

pub fn pretty(value: &J) -> String {
    let mut out = String::new();
    write_pretty(value, &mut out, 0);
    out.push('\n');
    out
}

fn write_pretty(value: &J, out: &mut String, indent: usize) {
    match value {
        J::Arr(items) if items.is_empty() => out.push_str("[]"),
        J::Obj(map) if map.is_empty() => out.push_str("{}"),
        J::Arr(items) => {
            out.push_str("[\n");
            for (i, item) in items.iter().enumerate() {
                push_indent(out, indent + 1);
                write_pretty(item, out, indent + 1);
                if i + 1 != items.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            push_indent(out, indent);
            out.push(']');
        }
        J::Obj(map) => {
            out.push_str("{\n");
            let len = map.len();
            for (i, (k, v)) in map.iter().enumerate() {
                push_indent(out, indent + 1);
                push_escaped(out, k);
                out.push_str(": ");
                write_pretty(v, out, indent + 1);
                if i + 1 != len {
                    out.push(',');
                }
                out.push('\n');
            }
            push_indent(out, indent);
            out.push('}');
        }
        other => other.write(out),
    }
}

fn push_indent(out: &mut String, n: usize) {
    for _ in 0..n {
        out.push_str("  ");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_and_sorts_keys() {
        let v = J::obj(vec![("b", J::str("a\"b")), ("a", J::Num(0.018))]);
        assert_eq!(v.render(), r#"{"a":0.018,"b":"a\"b"}"#);
    }
}
