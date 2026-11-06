// Copyright 2010 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.
//
// Changed representation: private iterative Rust spans for the fixed STUdio
// decoder. See source-manifest.json for original hashes, selections and notices.

use super::DecodeFailure;

const MAX_DEPTH: usize = 10_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Kind {
    Object,
    Array,
    String,
    Null,
    Other,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Span<'a> {
    pub raw: &'a [u8],
    pub kind: Kind,
}

#[derive(Clone, Copy)]
enum Frame {
    ArrayFirst,
    ArrayNext,
    ArrayAfter,
    ObjectFirst,
    ObjectKey,
    ObjectColon,
    ObjectAfter,
}

pub(super) fn whitespace(input: &[u8], mut pos: usize) -> usize {
    while input
        .get(pos)
        .is_some_and(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
    {
        pos += 1;
    }
    pos
}

fn syntax(offset: usize) -> DecodeFailure {
    DecodeFailure::Syntax { offset }
}

pub(super) fn string_end(input: &[u8], start: usize) -> Result<usize, DecodeFailure> {
    if input.get(start) != Some(&b'"') {
        return Err(syntax(start));
    }
    let mut pos = start + 1;
    while let Some(&byte) = input.get(pos) {
        pos += 1;
        match byte {
            b'"' => return Ok(pos),
            b'\\' => {
                let escape = input.get(pos).copied().ok_or_else(|| syntax(pos))?;
                pos += 1;
                match escape {
                    b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => {}
                    b'u' => {
                        for _ in 0..4 {
                            if !input.get(pos).is_some_and(u8::is_ascii_hexdigit) {
                                return Err(syntax(pos));
                            }
                            pos += 1;
                        }
                    }
                    _ => return Err(syntax(pos - 1)),
                }
            }
            0..=0x1f => return Err(syntax(pos - 1)),
            // Like Go's scanner, this stage does not reject raw invalid UTF8
            // or unpaired UTF16 escapes. Unquoting handles replacement later.
            _ => {}
        }
    }
    Err(syntax(pos))
}

fn number_end(input: &[u8], start: usize) -> Result<usize, DecodeFailure> {
    let mut pos = start;
    if input.get(pos) == Some(&b'-') {
        pos += 1;
    }
    match input.get(pos) {
        Some(b'0') => pos += 1,
        Some(b'1'..=b'9') => {
            pos += 1;
            while input.get(pos).is_some_and(u8::is_ascii_digit) {
                pos += 1;
            }
        }
        _ => return Err(syntax(pos)),
    }
    if input.get(pos) == Some(&b'.') {
        pos += 1;
        let first = pos;
        while input.get(pos).is_some_and(u8::is_ascii_digit) {
            pos += 1;
        }
        if pos == first {
            return Err(syntax(pos));
        }
    }
    if matches!(input.get(pos), Some(b'e' | b'E')) {
        pos += 1;
        if matches!(input.get(pos), Some(b'+' | b'-')) {
            pos += 1;
        }
        let first = pos;
        while input.get(pos).is_some_and(u8::is_ascii_digit) {
            pos += 1;
        }
        if pos == first {
            return Err(syntax(pos));
        }
    }
    Ok(pos)
}

/// Scan exactly the first complete value, not the rest of a Decoder stream.
/// Containers use an explicit bounded stack; no recursive Value tree is built.
pub(super) fn value(input: &[u8], offset: usize) -> Result<(Span<'_>, usize), DecodeFailure> {
    let start = whitespace(input, offset);
    let kind = match input.get(start) {
        Some(b'{') => Kind::Object,
        Some(b'[') => Kind::Array,
        Some(b'"') => Kind::String,
        Some(b'n') => Kind::Null,
        _ => Kind::Other,
    };
    let mut pos = start;
    let mut frames = Vec::new();
    let mut need_value = true;
    loop {
        pos = whitespace(input, pos);
        if need_value {
            match input.get(pos) {
                Some(b'{') => {
                    frames.push(Frame::ObjectFirst);
                    pos += 1;
                }
                Some(b'[') => {
                    frames.push(Frame::ArrayFirst);
                    pos += 1;
                }
                Some(b'"') => pos = string_end(input, pos)?,
                Some(b'n' | b't' | b'f') => {
                    let literal: &[u8] = match input[pos] {
                        b'n' => b"null",
                        b't' => b"true",
                        _ => b"false",
                    };
                    if !input[pos..].starts_with(literal) {
                        return Err(syntax(pos));
                    }
                    pos += literal.len();
                }
                Some(b'-' | b'0'..=b'9') => pos = number_end(input, pos)?,
                _ => return Err(syntax(pos)),
            }
            if frames.len() > MAX_DEPTH {
                return Err(syntax(pos - 1));
            }
            need_value = false;
        } else if let Some(frame) = frames.last_mut() {
            match *frame {
                Frame::ArrayFirst if input.get(pos) == Some(&b']') => {
                    frames.pop();
                    pos += 1;
                }
                Frame::ArrayFirst | Frame::ArrayNext => {
                    *frame = Frame::ArrayAfter;
                    need_value = true;
                }
                Frame::ArrayAfter => match input.get(pos) {
                    Some(b',') => {
                        *frame = Frame::ArrayNext;
                        pos += 1;
                    }
                    Some(b']') => {
                        frames.pop();
                        pos += 1;
                    }
                    _ => return Err(syntax(pos)),
                },
                Frame::ObjectFirst if input.get(pos) == Some(&b'}') => {
                    frames.pop();
                    pos += 1;
                }
                Frame::ObjectFirst | Frame::ObjectKey => {
                    pos = string_end(input, pos)?;
                    *frame = Frame::ObjectColon;
                }
                Frame::ObjectColon => {
                    if input.get(pos) != Some(&b':') {
                        return Err(syntax(pos));
                    }
                    pos += 1;
                    *frame = Frame::ObjectAfter;
                    need_value = true;
                }
                Frame::ObjectAfter => match input.get(pos) {
                    Some(b',') => {
                        *frame = Frame::ObjectKey;
                        pos += 1;
                    }
                    Some(b'}') => {
                        frames.pop();
                        pos += 1;
                    }
                    _ => return Err(syntax(pos)),
                },
            }
        }
        if frames.is_empty() && !need_value {
            return Ok((
                Span {
                    raw: &input[start..pos],
                    kind,
                },
                pos,
            ));
        }
    }
}

/// Iterate already syntax-checked container contents as borrowed raw spans.
pub(super) struct Cursor<'a> {
    input: &'a [u8],
    pos: usize,
    first: bool,
}

impl<'a> Cursor<'a> {
    pub fn new(container: Span<'a>) -> Self {
        Self {
            input: &container.raw[1..container.raw.len() - 1],
            pos: 0,
            first: true,
        }
    }

    fn next(&mut self) -> Result<bool, DecodeFailure> {
        self.pos = whitespace(self.input, self.pos);
        if self.pos == self.input.len() {
            return Ok(false);
        }
        if !self.first {
            if self.input.get(self.pos) != Some(&b',') {
                return Err(syntax(self.pos));
            }
            self.pos = whitespace(self.input, self.pos + 1);
        }
        self.first = false;
        Ok(true)
    }

    pub fn element(&mut self) -> Result<Option<Span<'a>>, DecodeFailure> {
        if !self.next()? {
            return Ok(None);
        }
        let (span, end) = value(self.input, self.pos)?;
        self.pos = end;
        Ok(Some(span))
    }

    pub fn member(&mut self) -> Result<Option<(&'a [u8], Span<'a>)>, DecodeFailure> {
        if !self.next()? {
            return Ok(None);
        }
        let end = string_end(self.input, self.pos)?;
        let key = &self.input[self.pos..end];
        self.pos = whitespace(self.input, end);
        if self.input.get(self.pos) != Some(&b':') {
            return Err(syntax(self.pos));
        }
        let (span, end) = value(self.input, self.pos + 1)?;
        self.pos = end;
        Ok(Some((key, span)))
    }
}
