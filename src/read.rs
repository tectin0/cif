//! Reading a CIF 1.1 document.

use crate::document::{Cif, DataBlock, Item, Loop, Style, Value};

/// Why a text isn't a CIF 1.1 document, and on which line.
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

/// Reads `text` as a CIF 1.1 document. A leading byte order mark is skipped, and a line may end
/// in `\r\n`.
pub fn read(text: &str) -> Result<Cif, ReadError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if text.starts_with("#\\#CIF_2.0") {
        return Err(ReadError {
            line: 1,
            message: "CIF 2.0 documents aren't supported".into(),
        });
    }

    let tokens = tokenize(text)?;
    Parser { tokens, next: 0 }.document()
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

fn tokenize(text: &str) -> Result<Vec<(Token, usize)>, ReadError> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut pos = 0;
    let mut line = 1;

    let line_end = |from: usize| text[from..].find('\n').map_or(text.len(), |i| from + i);
    let trim_cr = |s: &str| s.strip_suffix('\r').unwrap_or(s).to_string();

    while pos < bytes.len() {
        let c = bytes[pos];
        if c == b'\n' {
            line += 1;
            pos += 1;
            continue;
        }
        if c.is_ascii_whitespace() {
            pos += 1;
            continue;
        }

        let at_line_start = pos == 0 || bytes[pos - 1] == b'\n';
        let start_line = line;

        if c == b';' && at_line_start {
            let mut lines = vec![trim_cr(&text[pos + 1..line_end(pos)])];
            pos = line_end(pos);
            loop {
                if pos >= bytes.len() {
                    return Err(ReadError {
                        line: start_line,
                        message: "text field without its closing `;` line".into(),
                    });
                }
                pos += 1;
                line += 1;
                if bytes.get(pos) == Some(&b';') {
                    pos += 1;
                    break;
                }
                lines.push(trim_cr(&text[pos..line_end(pos)]));
                pos = line_end(pos);
            }
            tokens.push((
                Token::Value(Value {
                    text: lines.join("\n"),
                    style: Style::TextField,
                }),
                start_line,
            ));
            continue;
        }

        if c == b'#' {
            let end = line_end(pos);
            tokens.push((Token::Comment(trim_cr(&text[pos + 1..end])), start_line));
            pos = end;
            continue;
        }

        if c == b'\'' || c == b'"' {
            let end = line_end(pos);
            let closing = (pos + 1..end).find(|&i| {
                bytes[i] == c && bytes.get(i + 1).is_none_or(|next| next.is_ascii_whitespace())
            });
            let Some(closing) = closing else {
                return Err(ReadError {
                    line: start_line,
                    message: format!("quoted value without its closing {}", c as char),
                });
            };
            let style = if c == b'\'' {
                Style::SingleQuoted
            } else {
                Style::DoubleQuoted
            };
            tokens.push((
                Token::Value(Value {
                    text: text[pos + 1..closing].to_string(),
                    style,
                }),
                start_line,
            ));
            pos = closing + 1;
            continue;
        }

        let end = (pos..bytes.len())
            .find(|&i| bytes[i].is_ascii_whitespace())
            .unwrap_or(bytes.len());
        let word = &text[pos..end];
        pos = end;
        tokens.push((word_token(word, start_line)?, start_line));
    }

    Ok(tokens)
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
            message: format!("`{word}` is reserved and not part of CIF 1.1"),
        });
    } else if word == "?" {
        Token::Value(Value::unknown())
    } else if word == "." {
        Token::Value(Value::inapplicable())
    } else {
        Token::Value(Value {
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
                    return self.error(line, format!("value `{}` without a tag", value.text));
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
