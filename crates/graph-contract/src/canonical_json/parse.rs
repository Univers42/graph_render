//! A strict RFC 8259 JSON reader, dependency-free so the contract stays usable where
//! graph-core is. Refused as well as the grammar's own faults: a key repeated in one
//! object (which of the two would count is not defined), an unpaired UTF-16 surrogate
//! (it names no character), and nesting past [`MAX_DEPTH`] (a snapshot is four deep).
//! Numbers are kept as their text, so the reader of each field decides how to round.

#[cfg(test)]
mod differential;
#[cfg(test)]
mod mutate;
#[cfg(test)]
mod reference;
mod strings;

use super::JsonError;
use std::collections::BTreeSet;

/// Deepest nesting read.
pub const MAX_DEPTH: u32 = 32;

/// Member count above which an object's duplicate-key check builds a set of the keys it
/// has read instead of scanning them.
const WIDE_OBJECT: usize = 32;

/// A parsed JSON value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// `null`.
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// A number, as written; its text matches the JSON number grammar.
    Number(String),
    /// A string, unescaped.
    String(String),
    /// An array.
    Array(Vec<Value>),
    /// An object's members in document order, keys unique.
    Object(Vec<(String, Value)>),
}

/// Parses one JSON text: a value with only whitespace around it.
pub fn parse(text: &str) -> Result<Value, JsonError> {
    let mut p = Parser {
        text,
        at: 0,
        slots: Vec::new(),
        members: Vec::new(),
    };
    let value = p.value(0)?;
    p.skip_space();
    if p.at != text.len() {
        return Err(p.fault("text after the value"));
    }
    Ok(value)
}

pub(super) struct Parser<'a> {
    pub(super) text: &'a str,
    pub(super) at: usize,
    /// Array elements, and (key, value) pairs for objects, in the order they were read.
    /// Both are scratch for the whole parse: every container writes here and drains its
    /// own tail, so the `Vec` it hands back is allocated once at its exact length instead
    /// of growing by doubling. Two stacks rather than one because the element types
    /// differ; both peak at the document's widest single container.
    slots: Vec<Value>,
    members: Vec<(String, Value)>,
}

impl Parser<'_> {
    pub(super) fn fault(&self, what: &'static str) -> JsonError {
        let at = u32::try_from(self.at).unwrap_or(u32::MAX);
        JsonError::Syntax { at, what }
    }

    pub(super) fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.at).copied()
    }

    fn skip_space(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn eat(&mut self, byte: u8) -> bool {
        let found = self.peek() == Some(byte);
        self.at += usize::from(found);
        found
    }

    fn value(&mut self, depth: u32) -> Result<Value, JsonError> {
        if depth > MAX_DEPTH {
            return Err(self.fault("nested too deep"));
        }
        self.skip_space();
        match self.peek() {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => strings::read_string(self).map(Value::String),
            Some(b'-' | b'0'..=b'9') => self.number().map(Value::Number),
            Some(b't') => self.literal("true", Value::Bool(true)),
            Some(b'f') => self.literal("false", Value::Bool(false)),
            Some(b'n') => self.literal("null", Value::Null),
            Some(_) => Err(self.fault("not the start of a value")),
            None => Err(self.fault("the text ends where a value should be")),
        }
    }

    fn literal(&mut self, word: &str, value: Value) -> Result<Value, JsonError> {
        if !self.text[self.at..].starts_with(word) {
            return Err(self.fault("not the start of a value"));
        }
        self.at += word.len();
        Ok(value)
    }

    fn array(&mut self, depth: u32) -> Result<Value, JsonError> {
        self.at += 1;
        let base = self.slots.len();
        self.skip_space();
        if self.eat(b']') {
            return Ok(Value::Array(self.take_slots(base)));
        }
        loop {
            let item = self.value(depth + 1)?;
            self.slots.push(item);
            self.skip_space();
            if self.eat(b']') {
                return Ok(Value::Array(self.take_slots(base)));
            }
            if !self.eat(b',') {
                return Err(self.fault("expected , or ] in an array"));
            }
        }
    }

    fn object(&mut self, depth: u32) -> Result<Value, JsonError> {
        self.at += 1;
        let base = self.members.len();
        let mut wide: Option<BTreeSet<String>> = None;
        self.skip_space();
        if self.eat(b'}') {
            return Ok(Value::Object(self.take_members(base)));
        }
        loop {
            self.skip_space();
            if self.peek() != Some(b'"') {
                return Err(self.fault("expected a key"));
            }
            let key_at = self.at;
            let key = strings::read_string(self)?;
            self.skip_space();
            if !self.eat(b':') {
                return Err(self.fault("expected : after a key"));
            }
            let value = self.value(depth + 1)?;
            if self.key_seen(base, &mut wide, &key) {
                self.at = key_at;
                return Err(self.fault("a key repeated in one object"));
            }
            self.members.push((key, value));
            self.skip_space();
            if self.eat(b'}') {
                return Ok(Value::Object(self.take_members(base)));
            }
            if !self.eat(b',') {
                return Err(self.fault("expected , or } in an object"));
            }
        }
    }

    /// The members this object has read so far, as a `Vec` allocated once at its length.
    fn take_members(&mut self, base: usize) -> Vec<(String, Value)> {
        let mut out = Vec::with_capacity(self.members.len() - base);
        out.extend(self.members.drain(base..));
        out
    }

    /// The elements this array has read so far, likewise.
    fn take_slots(&mut self, base: usize) -> Vec<Value> {
        let mut out = Vec::with_capacity(self.slots.len() - base);
        out.extend(self.slots.drain(base..));
        out
    }

    /// Whether `key` is already one of this object's members. Narrow objects compare
    /// against the members on the scratch stack — no allocation, and the ten members of a
    /// node cost forty-five `memcmp`s. A wide one builds the set it always had: a
    /// quadratic over a hostile object is a worse trade than a clone per member.
    ///
    /// Ponytail: the switch is at [`WIDE_OBJECT`]; a document with more members than that
    /// in one object pays the old clone-per-key cost, and would go quadratic if the
    /// threshold were raised.
    fn key_seen(&self, base: usize, wide: &mut Option<BTreeSet<String>>, key: &str) -> bool {
        let seen = &self.members[base..];
        if seen.len() < WIDE_OBJECT {
            return seen.iter().any(|(k, _)| k == key);
        }
        let keys = wide.get_or_insert_with(|| {
            seen.iter()
                .map(|(k, _)| k.clone())
                .collect::<BTreeSet<String>>()
        });
        !keys.insert(key.to_owned())
    }

    fn number(&mut self) -> Result<String, JsonError> {
        let start = self.at;
        self.eat(b'-');
        if !self.eat(b'0') && self.digits() == 0 {
            return Err(self.fault("a number needs digits"));
        }
        if self.eat(b'.') && self.digits() == 0 {
            return Err(self.fault("a fraction needs digits"));
        }
        if self.eat(b'e') || self.eat(b'E') {
            let _ = self.eat(b'+') || self.eat(b'-');
            if self.digits() == 0 {
                return Err(self.fault("an exponent needs digits"));
            }
        }
        Ok(self.text[start..self.at].to_owned())
    }

    fn digits(&mut self) -> usize {
        let start = self.at;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.at += 1;
        }
        self.at - start
    }
}
