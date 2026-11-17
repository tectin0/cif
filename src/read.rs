//! Reading a CIF 1.1 or 2.0 document.

use std::ops::Range;

use crate::document::{
    Cif, DataBlock, Item, Loop, Style, TableEntry, Value, Version, V2_DELIMITERS,
};
use crate::spans::{BlockSpans, ItemSpans, Spans};

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
    read_with_spans(text).map(|(cif, _)| cif)
}

/// Reads `text` as [`read`] does, and tells where each part of the document is in `text`.
pub fn read_with_spans(text: &str) -> Result<(Cif, Spans), ReadError> {
    let unmarked = text.strip_prefix('\u{feff}').unwrap_or(text);
    let first_line = unmarked.lines().next().unwrap_or_default().trim_end();
    let (version, body, first_body_line) = if first_line == V2_MAGIC {
        let rest = unmarked
            .find('\n')
            .map_or("", |newline| &unmarked[newline + 1..]);
        (Version::V2_0, rest, 2)
    } else {
        (Version::V1_1, unmarked, 1)
    };

    let mut lexer = Lexer {
        text: body,
        base: text.len() - body.len(),
        pos: 0,
        line: first_body_line,
        version,
        after_blank_line: Vec::new(),
    };
    let mut tokens = Vec::new();
    while let Some(token) = lexer.next_token()? {
        tokens.push(token);
    }
    let parser = Parser {
        tokens,
        next: 0,
        after_blank_line: lexer.after_blank_line,
    };
    let (mut cif, spans) = parser.document()?;
    cif.version = version;
    Ok((cif, spans))
}

/// Reads `text` as a single value of a document of `version`, as it would be written after a tag.
pub fn read_value(text: &str, version: Version) -> Result<Value, ReadError> {
    let mut lexer = Lexer {
        text,
        base: 0,
        pos: 0,
        line: 1,
        version,
        after_blank_line: Vec::new(),
    };
    lexer.skip_whitespace(false);
    if lexer.peek().is_none() {
        return lexer.error(1, "no value");
    }
    let value = lexer.value()?;
    lexer.skip_whitespace(false);
    if lexer.peek().is_some() {
        return lexer.error(lexer.line, "more than one value");
    }
    Ok(value)
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
    /// Where the tokens start that follow a blank line, in order.
    after_blank_line: Vec<usize>,
    /// Where `text` starts in the text being read, which ranges count from.
    base: usize,
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
        self.text[from..]
            .find('\n')
            .map_or(self.text.len(), |i| from + i)
    }

    /// Skips whitespace, counting lines; inside a list or table (`skip_comments`) comments too.
    /// Answers how many line breaks it skipped.
    fn skip_whitespace(&mut self, skip_comments: bool) -> usize {
        let mut line_breaks = 0;
        while let Some(c) = self.peek() {
            if c == b'\n' {
                self.line += 1;
                self.pos += 1;
                line_breaks += 1;
            } else if c.is_ascii_whitespace() {
                self.pos += 1;
            } else if skip_comments && c == b'#' {
                self.pos = self.line_end(self.pos);
            } else {
                break;
            }
        }
        line_breaks
    }

    /// The next token, its line and its range.
    fn next_token(&mut self) -> Result<Option<(Token, usize, Range<usize>)>, ReadError> {
        let line_breaks = self.skip_whitespace(false);
        let Some(c) = self.peek() else {
            return Ok(None);
        };
        let line = self.line;
        let start = self.pos;
        if line_breaks >= 2 {
            self.after_blank_line.push(self.base + start);
        }

        let token = if c == b'#' {
            let end = self.line_end(self.pos);
            let comment = trim_cr(&self.text[self.pos + 1..end]);
            self.pos = end;
            Token::Comment(comment)
        } else if (c == b';' && self.at_line_start())
            || c == b'\''
            || c == b'"'
            || (self.version == Version::V2_0 && (c == b'[' || c == b'{'))
        {
            Token::Value(self.value()?)
        } else if self.version == Version::V2_0 && (c == b']' || c == b'}') {
            return self.error(line, format!("`{}` closes no list or table", c as char));
        } else {
            let word = self.word();
            word_token(word, line)?
        };

        let range = self.base + start..self.base + self.pos;
        Ok(Some((token, line, range)))
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
                bytes[i] == quote
                    && bytes
                        .get(i + 1)
                        .is_none_or(|next| next.is_ascii_whitespace())
            })
        };
        let Some(closing) = closing else {
            return self.error(
                line,
                format!("quoted value without its closing {}", quote as char),
            );
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
    tokens: Vec<(Token, usize, Range<usize>)>,
    next: usize,
    /// Where the tokens start that follow a blank line, in order.
    after_blank_line: Vec<usize>,
}

/// A pair's or loop's comments, which the document keeps as items after it, and their ranges.
type Comments = (Vec<Item>, Vec<ItemSpans>);

/// A block's or save frame's items, their ranges, and the range of a save frame's closing `save_`.
type Items = (Vec<Item>, Vec<ItemSpans>, Range<usize>);

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.next).map(|(token, _, _)| token)
    }

    fn take(&mut self) -> Option<(Token, usize, Range<usize>)> {
        let token = self.tokens.get(self.next).cloned();
        self.next += 1;
        token
    }

    fn line(&self) -> usize {
        self.tokens
            .get(self.next)
            .or(self.tokens.last())
            .map_or(1, |(_, line, _)| *line)
    }

    fn error<T>(&self, line: usize, message: impl Into<String>) -> Result<T, ReadError> {
        Err(ReadError {
            line,
            message: message.into(),
        })
    }

    fn document(mut self) -> Result<(Cif, Spans), ReadError> {
        let mut cif = Cif::default();
        let mut spans = Spans::default();

        while let Some((token, line, range)) = self.take() {
            match token {
                Token::Comment(text) if cif.blocks.is_empty() => {
                    cif.comments.push(text);
                    spans.comments.push(range);
                }
                Token::Data(name) => {
                    let (comments, comment_ranges) =
                        match (cif.blocks.last_mut(), spans.blocks.last_mut()) {
                            (Some(before), Some(before_spans)) => {
                                self.introduction(before, before_spans)
                            }
                            _ => (Vec::new(), Vec::new()),
                        };
                    let (items, item_spans, _) = self.items(false)?;
                    cif.blocks.push(DataBlock {
                        comments,
                        name,
                        items,
                    });
                    spans.blocks.push(BlockSpans {
                        comments: comment_ranges,
                        header: range,
                        items: item_spans,
                    });
                }
                _ => return self.error(line, "content before the first data block"),
            }
        }

        Ok((cif, spans))
    }

    /// The comments that end `block` after a blank line, taken out of it, and their ranges: they
    /// introduce the block after it. Comments right after its last other item stay its own.
    fn introduction(
        &self,
        block: &mut DataBlock,
        spans: &mut BlockSpans,
    ) -> (Vec<String>, Vec<Range<usize>>) {
        let is_comment = |spans: &ItemSpans| matches!(spans, ItemSpans::Comment(_));
        // A comment written among a pair's or loop's values is an item after it, within its text.
        let content_end = spans
            .items
            .iter()
            .filter(|item| !is_comment(item))
            .map(|item| item.whole().end)
            .max()
            .unwrap_or(spans.header.end);
        let trailing = spans
            .items
            .iter()
            .rposition(
                |item| !matches!(item, ItemSpans::Comment(range) if range.start > content_end),
            )
            .map_or(0, |last| last + 1);
        let Some(first) = (trailing..spans.items.len()).find(|&index| {
            matches!(&spans.items[index], ItemSpans::Comment(range)
                if self.after_blank_line.binary_search(&range.start).is_ok())
        }) else {
            return (Vec::new(), Vec::new());
        };

        let comments = block
            .items
            .drain(first..)
            .filter_map(|item| match item {
                Item::Comment(text) => Some(text),
                _ => None,
            })
            .collect();
        let ranges = spans
            .items
            .drain(first..)
            .filter_map(|item| match item {
                ItemSpans::Comment(range) => Some(range),
                _ => None,
            })
            .collect();
        (comments, ranges)
    }

    /// The items up to the next data block (or the end), or, in a save frame, up to its `save_`,
    /// their ranges, and the range of that `save_`.
    fn items(&mut self, in_frame: bool) -> Result<Items, ReadError> {
        let mut items = Vec::new();
        let mut spans = Vec::new();

        loop {
            match self.peek() {
                None | Some(Token::Data(_)) => {
                    if in_frame {
                        return self.error(self.line(), "save frame without its closing `save_`");
                    }
                    return Ok((items, spans, 0..0));
                }
                Some(Token::SaveEnd) => {
                    if !in_frame {
                        return self.error(self.line(), "`save_` outside a save frame");
                    }
                    let (_, _, end) = self.take().expect("peeked");
                    return Ok((items, spans, end));
                }
                _ => {}
            }

            let (token, line, range) = self.take().expect("peeked");
            match token {
                Token::Comment(text) => {
                    items.push(Item::Comment(text));
                    spans.push(ItemSpans::Comment(range));
                }
                Token::Tag(tag) => {
                    let mut comments: Comments = (Vec::new(), Vec::new());
                    let (value, value_range) = loop {
                        match self.take() {
                            Some((Token::Comment(text), _, comment)) => {
                                comments.0.push(Item::Comment(text));
                                comments.1.push(ItemSpans::Comment(comment));
                            }
                            Some((Token::Value(value), _, value_range)) => {
                                break (value, value_range)
                            }
                            _ => return self.error(line, format!("`{tag}` without a value")),
                        }
                    };
                    items.push(Item::Pair { tag, value });
                    spans.push(ItemSpans::Pair {
                        tag: range,
                        value: value_range,
                    });
                    items.extend(comments.0);
                    spans.extend(comments.1);
                }
                Token::Loop => {
                    let (l, loop_spans, comments) = self.loop_body(line, range)?;
                    items.push(Item::Loop(l));
                    spans.push(loop_spans);
                    items.extend(comments.0);
                    spans.extend(comments.1);
                }
                Token::Save(name) => {
                    if in_frame {
                        return self.error(line, "save frame inside a save frame");
                    }
                    let (frame_items, frame_spans, end) = self.items(true)?;
                    items.push(Item::SaveFrame {
                        name,
                        items: frame_items,
                    });
                    spans.push(ItemSpans::SaveFrame {
                        header: range,
                        items: frame_spans,
                        end,
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

    /// The loop after `loop_` (at `keyword`), its ranges, and the comments among its tags and
    /// values.
    fn loop_body(
        &mut self,
        line: usize,
        keyword: Range<usize>,
    ) -> Result<(Loop, ItemSpans, Comments), ReadError> {
        let mut tags = Vec::new();
        let mut tag_ranges = Vec::new();
        let mut comments: Comments = (Vec::new(), Vec::new());
        loop {
            match self.peek() {
                Some(Token::Tag(_)) => {
                    let Some((Token::Tag(tag), _, range)) = self.take() else {
                        unreachable!("peeked a tag")
                    };
                    tags.push(tag);
                    tag_ranges.push(range);
                }
                Some(Token::Comment(_)) => {
                    let Some((Token::Comment(text), _, range)) = self.take() else {
                        unreachable!("peeked a comment")
                    };
                    comments.0.push(Item::Comment(text));
                    comments.1.push(ItemSpans::Comment(range));
                }
                _ => break,
            }
        }
        if tags.is_empty() {
            return self.error(line, "`loop_` without tags");
        }

        let mut values = Vec::new();
        let mut value_ranges = Vec::new();
        loop {
            match self.peek() {
                Some(Token::Value(_)) => {
                    let Some((Token::Value(value), _, range)) = self.take() else {
                        unreachable!("peeked a value")
                    };
                    values.push(value);
                    value_ranges.push(range);
                }
                Some(Token::Comment(_)) => {
                    let Some((Token::Comment(text), _, range)) = self.take() else {
                        unreachable!("peeked a comment")
                    };
                    comments.0.push(Item::Comment(text));
                    comments.1.push(ItemSpans::Comment(range));
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
        let spans = ItemSpans::Loop {
            keyword,
            tags: tag_ranges,
            values: value_ranges,
        };
        Ok((Loop { tags, rows }, spans, comments))
    }
}
