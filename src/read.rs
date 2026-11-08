//! Reading a CIF 1.1 or 2.0 document.

use crate::document::{
    Cif, DataBlock, Item, Loop, Style, TableEntry, Value, Version, V2_DELIMITERS,
};

/// Why a text isn't a CIF document, and on which line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadError {
    /// The line, counting from 1.
    pub line: usize,
    /// What is wrong there.
    pub message: String,
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ReadError {}

/// The first line of a CIF 2.0 document.
pub(crate) const V2_MAGIC: &str = "#\\#CIF_2.0";

/// Reads `text` as a CIF document: CIF 2.0 if its first line is `#\#CIF_2.0`, else CIF 1.1. A
/// leading byte order mark is skipped, and a line may end in `\r\n`.
pub fn read(text: &str) -> Result<Cif, ReadError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let first_line = text.lines().next().unwrap_or_default().trim_end();
    let (version, body, first_body_line) = if first_line == V2_MAGIC {
        let rest = text.find('\n').map_or("", |newline| &text[newline + 1..]);
        (Version::V2_0, rest, 2)
    } else {
        (Version::V1_1, text, 1)
    };

    let mut lexer = Lexer {
        text: body,
        pos: 0,
        line: first_body_line,
        version,
    };
    let mut tokens = Vec::new();
    while let Some(token) = lexer.next_token()? {
        tokens.push(token);
    }
    let mut cif = Parser { tokens, next: 0 }.document()?;
    cif.version = version;
    Ok(cif)
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Comment(String),
    Data(String),
    Save(String),
    SaveEnd,
    Loop,
    Tag(String),
    Value(Value),
}

struct Lexer<'a> {
    text: &'a str,
    pos: usize,
    line: usize,
    version: Version,
}

impl Lexer<'_> {
    fn error<T>(&self, line: usize, message: impl Into<String>) -> Result<T, ReadError> {
        Err(ReadError {
            line,
            message: message.into(),
        })
    }

    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.pos).copied()
    }

    fn at_line_start(&self) -> bool {
        self.pos == 0 || self.text.as_bytes()[self.pos - 1] == b'\n'
    }

    fn line_end(&self, from: usize) -> usize {
        self.text[from..].find('\n').map_or(self.text.len(), |i| from + i)
    }

    /// Skips whitespace, counting lines; inside a list or table (`skip_comments`) comments too.
    fn skip_whitespace(&mut self, skip_comments: bool) {
        while let Some(c) = self.peek() {
            if c == b'\n' {
                self.line += 1;
                self.pos += 1;
            } else if c.is_ascii_whitespace() {
                self.pos += 1;
            } else if skip_comments && c == b'#' {
                self.pos = self.line_end(self.pos);
            } else {
                break;
            }
        }
    }

    fn next_token(&mut self) -> Result<Option<(Token, usize)>, ReadError> {
        self.skip_whitespace(false);
        let Some(c) = self.peek() else {
            return Ok(None);
        };
        let line = self.line;

        if c == b'#' {
            let end = self.line_end(self.pos);
            let comment = trim_cr(&self.text[self.pos + 1..end]);
            self.pos = end;
            return Ok(Some((Token::Comment(comment), line)));
        }

        let starts_value = (c == b';' && self.at_line_start())
            || c == b'\''
            || c == b'"'
            || (self.version == Version::V2_0 && (c == b'[' || c == b'{'));
        if starts_value {
            return Ok(Some((Token::Value(self.value()?), line)));
        }
        if self.version == Version::V2_0 && (c == b']' || c == b'}') {
            return self.error(line, format!("`{}` closes no list or table", c as char));
        }

        let word = self.word();
        Ok(Some((word_token(word, line)?, line)))
    }

    /// A bare word: up to whitespace, or in CIF 2.0 also up to a bracket or brace.
    fn word(&mut self) -> &str {
        let start = self.pos;
        let v2 = self.version == Version::V2_0;
        while let Some(c) = self.peek() {
            if c.is_ascii_whitespace() || (v2 && V2_DELIMITERS.contains(&(c as char))) {
                break;
            }
            self.pos += 1;
        }
        &self.text[start..self.pos]
    }

    /// The value at the current position: a text field, a quoted text, a list, a table, or a bare
    /// word.
    fn value(&mut self) -> Result<Value, ReadError> {
        let line = self.line;
        let c = self.peek().expect("called at a value");
        let v2 = self.version == Version::V2_0;

        if c == b';' && self.at_line_start() {
            return self.text_field();
        }
        if c == b'\'' || c == b'"' {
            return self.quoted();
        }
        if v2 && c == b'[' {
            return self.list();
        }
        if v2 && c == b'{' {
            return self.table();
        }

        let word = self.word().to_string();
        match word.as_str() {
            "" => self.error(line, format!("`{}` where a value belongs", c as char)),
            "?" => Ok(Value::unknown()),
            "." => Ok(Value::inapplicable()),
            _ if word.starts_with('_') || crate::document::is_reserved(&word) => {
                self.error(line, format!("`{word}` can't stand inside a list or table"))
            }
            _ => Ok(Value::Text {
                text: word,
                style: Style::Bare,
            }),
        }
    }

    fn text_field(&mut self) -> Result<Value, ReadError> {
        let start_line = self.line;
        let mut lines = vec![trim_cr(&self.text[self.pos + 1..self.line_end(self.pos)])];
        self.pos = self.line_end(self.pos);
        loop {
            if self.pos >= self.text.len() {
                return self.error(start_line, "text field without its closing `;` line");
            }
            self.pos += 1;
            self.line += 1;
            if self.peek() == Some(b';') {
                self.pos += 1;
                break;
            }
            lines.push(trim_cr(&self.text[self.pos..self.line_end(self.pos)]));
            self.pos = self.line_end(self.pos);
        }
        Ok(Value::Text {
            text: lines.join("\n"),
            style: Style::TextField,
        })
    }

    fn quoted(&mut self) -> Result<Value, ReadError> {
        let line = self.line;
        let bytes = self.text.as_bytes();
        let quote = bytes[self.pos];

        let delimiter = if quote == b'\'' { "'''" } else { "\"\"\"" };
        if self.version == Version::V2_0 && self.text[self.pos..].starts_with(delimiter) {
            let start = self.pos + 3;
            let Some(length) = self.text[start..].find(delimiter) else {
                return self.error(line, format!("text without its closing {delimiter}"));
            };
            let text = self.text[start..start + length].replace("\r\n", "\n");
            self.line += text.matches('\n').count();
            self.pos = start + length + 3;
            let style = if quote == b'\'' {
                Style::TripleSingleQuoted
            } else {
                Style::TripleDoubleQuoted
            };
            return Ok(Value::Text { text, style });
        }

        let end = self.line_end(self.pos);
        let closing = if self.version == Version::V2_0 {
            (self.pos + 1..end).find(|&i| bytes[i] == quote)
        } else {
            (self.pos + 1..end).find(|&i| {
                bytes[i] == quote && bytes.get(i + 1).is_none_or(|next| next.is_ascii_whitespace())
            })
        };
        let Some(closing) = closing else {
            return self.error(line, format!("quoted value without its closing {}", quote as char));
        };
        let text = self.text[self.pos + 1..closing].to_string();
        self.pos = closing + 1;
        let style = if quote == b'\'' {
            Style::SingleQuoted
        } else {
            Style::DoubleQuoted
        };
        Ok(Value::Text { text, style })
    }

    fn list(&mut self) -> Result<Value, ReadError> {
        let line = self.line;
        self.pos += 1;
        let mut values = Vec::new();
        loop {
            self.skip_whitespace(true);
            match self.peek() {
                None => return self.error(line, "list without its closing `]`"),
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Value::List(values));
                }
                Some(b'}') => return self.error(self.line, "`}` inside a list"),
                Some(_) => values.push(self.value()?),
            }
        }
    }

    fn table(&mut self) -> Result<Value, ReadError> {
        let line = self.line;
        self.pos += 1;
        let mut entries = Vec::new();
        loop {
            self.skip_whitespace(true);
            match self.peek() {
                None => return self.error(line, "table without its closing `}`"),
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Value::Table(entries));
                }
                Some(b'\'' | b'"') => {
                    let key_line = self.line;
                    let Value::Text { text: key, style } = self.quoted()? else {
                        unreachable!("a quoted value is text")
                    };
                    if self.peek() != Some(b':') {
                        return self.error(key_line, format!("table key '{key}' without its `:`"));
                    }
                    self.pos += 1;
                    self.skip_whitespace(true);
                    if self.peek().is_none_or(|c| c == b'}') {
                        return self.error(key_line, format!("table key '{key}' without a value"));
                    }
                    let value = self.value()?;
                    entries.push(TableEntry {
                        key,
                        key_style: style,
                        value,
                    });
                }
                Some(_) => return self.error(self.line, "a table's key must be quoted"),
            }
        }
    }
}

fn trim_cr(s: &str) -> String {
    s.strip_suffix('\r').unwrap_or(s).to_string()
}

fn word_token(word: &str, line: usize) -> Result<Token, ReadError> {
    let lower = word.to_ascii_lowercase();
    let token = if word.starts_with('_') {
        Token::Tag(word.to_string())
    } else if lower.starts_with("data_") {
        if word.len() == 5 {
            return Err(ReadError {
                line,
                message: "data block without a name".into(),
            });
        }
        Token::Data(word[5..].to_string())
    } else if lower.starts_with("save_") {
        if word.len() == 5 {
            Token::SaveEnd
        } else {
            Token::Save(word[5..].to_string())
        }
    } else if lower == "loop_" {
        Token::Loop
    } else if lower == "global_" || lower == "stop_" {
        return Err(ReadError {
            line,
            message: format!("`{word}` is reserved and not part of CIF"),
        });
    } else if word == "?" {
        Token::Value(Value::unknown())
    } else if word == "." {
        Token::Value(Value::inapplicable())
    } else {
        Token::Value(Value::Text {
            text: word.to_string(),
            style: Style::Bare,
        })
    };
    Ok(token)
}

struct Parser {
    tokens: Vec<(Token, usize)>,
    next: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.next).map(|(token, _)| token)
    }

    fn take(&mut self) -> Option<(Token, usize)> {
        let token = self.tokens.get(self.next).cloned();
        self.next += 1;
        token
    }

    fn line(&self) -> usize {
        self.tokens
            .get(self.next)
            .or(self.tokens.last())
            .map_or(1, |(_, line)| *line)
    }

    fn error<T>(&self, line: usize, message: impl Into<String>) -> Result<T, ReadError> {
        Err(ReadError {
            line,
            message: message.into(),
        })
    }

    fn document(mut self) -> Result<Cif, ReadError> {
        let mut cif = Cif::default();

        while let Some((token, line)) = self.take() {
            match token {
                Token::Comment(text) if cif.blocks.is_empty() => cif.comments.push(text),
                Token::Data(name) => {
                    let items = self.items(false)?;
                    cif.blocks.push(DataBlock { name, items });
                }
                _ => return self.error(line, "content before the first data block"),
            }
        }

        Ok(cif)
    }

    /// The items up to the next data block (or the end), or, in a save frame, up to its `save_`.
    fn items(&mut self, in_frame: bool) -> Result<Vec<Item>, ReadError> {
        let mut items = Vec::new();

        loop {
            match self.peek() {
                None | Some(Token::Data(_)) => {
                    if in_frame {
                        return self.error(self.line(), "save frame without its closing `save_`");
                    }
                    return Ok(items);
                }
                Some(Token::SaveEnd) => {
                    if !in_frame {
                        return self.error(self.line(), "`save_` outside a save frame");
                    }
                    self.take();
                    return Ok(items);
                }
                _ => {}
            }

            let (token, line) = self.take().expect("peeked");
            match token {
                Token::Comment(text) => items.push(Item::Comment(text)),
                Token::Tag(tag) => {
                    let mut comments = Vec::new();
                    let value = loop {
                        match self.take() {
                            Some((Token::Comment(text), _)) => comments.push(Item::Comment(text)),
                            Some((Token::Value(value), _)) => break value,
                            _ => return self.error(line, format!("`{tag}` without a value")),
                        }
                    };
                    items.push(Item::Pair { tag, value });
                    items.extend(comments);
                }
                Token::Loop => {
                    let (l, comments) = self.loop_body(line)?;
                    items.push(Item::Loop(l));
                    items.extend(comments);
                }
                Token::Save(name) => {
                    if in_frame {
                        return self.error(line, "save frame inside a save frame");
                    }
                    let frame_items = self.items(true)?;
                    items.push(Item::SaveFrame {
                        name,
                        items: frame_items,
                    });
                }
                Token::Value(value) => {
                    let shown = value.text().unwrap_or("a list or table").to_string();
                    return self.error(line, format!("value `{shown}` without a tag"));
                }
                Token::Data(_) | Token::SaveEnd => unreachable!("handled before taking"),
            }
        }
    }

    fn loop_body(&mut self, line: usize) -> Result<(Loop, Vec<Item>), ReadError> {
        let mut tags = Vec::new();
        let mut comments = Vec::new();
        loop {
            match self.peek() {
                Some(Token::Tag(_)) => {
                    let Some((Token::Tag(tag), _)) = self.take() else {
                        unreachable!("peeked a tag")
                    };
                    tags.push(tag);
                }
                Some(Token::Comment(_)) => {
                    let Some((Token::Comment(text), _)) = self.take() else {
                        unreachable!("peeked a comment")
                    };
                    comments.push(Item::Comment(text));
                }
                _ => break,
            }
        }
        if tags.is_empty() {
            return self.error(line, "`loop_` without tags");
        }

        let mut values = Vec::new();
        loop {
            match self.peek() {
                Some(Token::Value(_)) => {
                    let Some((Token::Value(value), _)) = self.take() else {
                        unreachable!("peeked a value")
                    };
                    values.push(value);
                }
                Some(Token::Comment(_)) => {
                    let Some((Token::Comment(text), _)) = self.take() else {
                        unreachable!("peeked a comment")
                    };
                    comments.push(Item::Comment(text));
                }
                _ => break,
            }
        }
        if values.is_empty() {
            return self.error(line, "`loop_` without values");
        }
        if values.len() % tags.len() != 0 {
            return self.error(
                line,
                format!(
                    "loop of {} tags holds {} values, not a whole number of rows",
                    tags.len(),
                    values.len()
                ),
            );
        }

        let width = tags.len();
        let mut rows = Vec::with_capacity(values.len() / width);
        let mut values = values.into_iter();
        while values.len() > 0 {
            rows.push(values.by_ref().take(width).collect());
        }
        Ok((Loop { tags, rows }, comments))
    }
}
