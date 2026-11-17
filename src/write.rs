//! Writing a CIF 1.1 or 2.0 document.
//!
//! A document is written in its version, except that a CIF 1.1 document holding a list or table is
//! written as CIF 2.0, the only version that can hold one. Every text value is written in its own
//! style when that style can hold its text in the written version, else in the plainest one that
//! can. A pair is written on one line (a text field on the lines after its tag), a loop with one
//! tag per line and one row per line, every row indented, so no line starts with a bare value.

use std::fmt::{self, Display, Formatter, Write};
use std::ops::Range;

use crate::document::{Cif, Item, Style, TableEntry, Value, Version};
use crate::read::V2_MAGIC;
use crate::spans::{BlockSpans, ItemSpans, Spans};

/// What writing answers: what was written where, or the writer's error.
type Written<T> = Result<T, fmt::Error>;

impl Display for Cif {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Out::new(f, self.written_version(), true, false)
            .document(self)
            .map(|_| ())
    }
}

impl Cif {
    /// The version the document is written in: its own, or CIF 2.0 for a CIF 1.1 document holding
    /// a list or table, which only CIF 2.0 can hold.
    fn written_version(&self) -> Version {
        if self.version == Version::V1_1 && self.blocks.iter().any(holds_compound) {
            Version::V2_0
        } else {
            self.version
        }
    }

    /// The document written, as [`ToString::to_string`] writes it, and where each of its parts is
    /// in the text, as reading the text with [`crate::read_with_spans`] tells.
    pub fn written_with_spans(&self) -> (String, Spans) {
        let mut text = String::new();
        let written = Out::new(&mut text, self.written_version(), true, false).document(self);
        match written {
            Ok(spans) => (text, spans),
            Err(_) => unreachable!("a `String` takes every write"),
        }
    }
}

impl Display for Value {
    /// The value as CIF 2.0 writes it.
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Out::new(f, Version::V2_0, false, true)
            .value(self)
            .map(|_| ())
    }
}

impl Item {
    /// The item as a document of `version` writes it, from the start of a line to a line break.
    pub fn written(&self, version: Version) -> String {
        WrittenItem(self, version).to_string()
    }
}

impl Value {
    /// The value as a document of `version` writes it after a tag or another value, without a
    /// space before it. A text field starts on a line of its own, so it starts with a line break
    /// unless it is written at the start of a line (`at_line_start`).
    pub fn written(&self, version: Version, at_line_start: bool) -> String {
        WrittenValue(self, version, at_line_start).to_string()
    }
}

struct WrittenItem<'a>(&'a Item, Version);

impl Display for WrittenItem<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Out::new(f, self.1, true, false)
            .items(std::slice::from_ref(self.0))
            .map(|_| ())
    }
}

struct WrittenValue<'a>(&'a Value, Version, bool);

impl Display for WrittenValue<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Out::new(f, self.1, self.2, true).value(self.0).map(|_| ())
    }
}

fn holds_compound(block: &crate::DataBlock) -> bool {
    fn holds(item: &Item) -> bool {
        match item {
            Item::Pair { value, .. } => value.is_compound(),
            Item::Loop(l) => l.rows.iter().flatten().any(Value::is_compound),
            Item::SaveFrame { items, .. } => items.iter().any(holds),
            Item::Comment(_) => false,
        }
    }
    block.items.iter().any(holds)
}

/// The written text and where it stands: how many bytes are written, and whether at the start of
/// a line or right after a token that the next one is glued to (an opening bracket or brace, a
/// table key's colon).
struct Out<'a> {
    f: &'a mut dyn Write,
    written: usize,
    version: Version,
    at_line_start: bool,
    glued: bool,
}

impl<'a> Out<'a> {
    fn new(f: &'a mut dyn Write, version: Version, at_line_start: bool, glued: bool) -> Self {
        Self {
            f,
            written: 0,
            version,
            at_line_start,
            glued,
        }
    }

    /// Writes `text`, leaving where the line stands to the caller.
    fn put(&mut self, text: &str) -> fmt::Result {
        self.f.write_str(text)?;
        self.written += text.len();
        Ok(())
    }

    /// Writes `text` as it is, where the line stands, and answers where it went.
    fn raw(&mut self, text: &str) -> Written<Range<usize>> {
        let start = self.written;
        self.put(text)?;
        self.at_line_start = false;
        self.glued = false;
        Ok(start..self.written)
    }

    fn newline(&mut self) -> fmt::Result {
        self.put("\n")?;
        self.at_line_start = true;
        self.glued = false;
        Ok(())
    }

    /// Writes `token` after a space, unless it starts the line or follows a glued token.
    fn token(&mut self, token: &str) -> Written<Range<usize>> {
        if !self.at_line_start && !self.glued {
            self.put(" ")?;
        }
        self.raw(token)
    }

    /// Writes `text` as comment lines, each on a line of its own, and answers where they went,
    /// without the last line break.
    fn comment(&mut self, text: &str) -> Written<Range<usize>> {
        if !self.at_line_start {
            self.newline()?;
        }
        let start = self.written;
        for line in text.split('\n') {
            self.raw(&format!("#{line}"))?;
            self.newline()?;
        }
        Ok(start..self.written - 1)
    }

    fn document(&mut self, cif: &Cif) -> Written<Spans> {
        let mut spans = Spans::default();

        if self.version == Version::V2_0 {
            self.raw(V2_MAGIC)?;
            self.newline()?;
        }
        // Comments before the first block are the document's, so the first block's are written
        // as such: they read back as the document's, after its own.
        let first_block_comments = cif.blocks.first().map_or(&[][..], |block| &block.comments);
        for comment in cif.comments.iter().chain(first_block_comments) {
            spans.comments.push(self.comment(comment)?);
        }
        for (index, block) in cif.blocks.iter().enumerate() {
            if index > 0 || !spans.comments.is_empty() {
                self.newline()?;
            }
            // After the blank line, so that they read back as the block's own.
            let comments = match index {
                0 => Vec::new(),
                _ => block
                    .comments
                    .iter()
                    .map(|comment| self.comment(comment))
                    .collect::<Written<Vec<_>>>()?,
            };
            let header = self.raw(&format!("data_{}", block.name))?;
            self.newline()?;
            let items = self.items(&block.items)?;
            spans.blocks.push(BlockSpans {
                comments,
                header,
                items,
            });
        }
        Ok(spans)
    }

    fn items(&mut self, items: &[Item]) -> Written<Vec<ItemSpans>> {
        let mut spans = Vec::with_capacity(items.len());
        for item in items {
            let item_spans = match item {
                Item::Pair { tag, value } => {
                    let tag = self.raw(tag)?;
                    let value = self.value(value)?;
                    self.newline()?;
                    ItemSpans::Pair { tag, value }
                }
                Item::Loop(l) => {
                    let keyword = self.raw("loop_")?;
                    self.newline()?;
                    let mut tags = Vec::with_capacity(l.tags.len());
                    for tag in &l.tags {
                        self.put("  ")?;
                        tags.push(self.raw(tag)?);
                        self.newline()?;
                    }
                    let mut values = Vec::with_capacity(l.tags.len() * l.rows.len());
                    for row in &l.rows {
                        self.raw(" ")?;
                        for value in row {
                            values.push(self.value(value)?);
                        }
                        self.newline()?;
                    }
                    ItemSpans::Loop {
                        keyword,
                        tags,
                        values,
                    }
                }
                Item::SaveFrame { name, items } => {
                    let header = self.raw(&format!("save_{name}"))?;
                    self.newline()?;
                    let items = self.items(items)?;
                    let end = self.raw("save_")?;
                    self.newline()?;
                    ItemSpans::SaveFrame { header, items, end }
                }
                Item::Comment(text) => ItemSpans::Comment(self.comment(text)?),
            };
            spans.push(item_spans);
        }
        Ok(spans)
    }

    /// The style `text` is written in: `style`, or the plainest one that can hold it.
    fn style_for(&self, text: &str, style: Style) -> Style {
        if style.can_hold(text, self.version) {
            style
        } else {
            Style::plainest_in(text, self.version)
        }
    }

    fn value(&mut self, value: &Value) -> Written<Range<usize>> {
        match value {
            Value::Text { text, style } => {
                let style = self.style_for(text, *style);
                if style == Style::TextField {
                    self.text_field(text)
                } else {
                    self.token(&quoted(text, style))
                }
            }
            Value::List(values) => {
                let start = self.token("[")?.start;
                self.glued = true;
                for value in values {
                    self.value(value)?;
                }
                Ok(start..self.raw("]")?.end)
            }
            Value::Table(entries) => {
                let start = self.token("{")?.start;
                self.glued = true;
                for TableEntry {
                    key,
                    key_style,
                    value,
                } in entries
                {
                    let key_style =
                        if key_style.is_quoted() && key_style.can_hold(key, self.version) {
                            *key_style
                        } else {
                            [
                                Style::SingleQuoted,
                                Style::DoubleQuoted,
                                Style::TripleSingleQuoted,
                                Style::TripleDoubleQuoted,
                            ]
                            .into_iter()
                            .find(|style| style.can_hold(key, self.version))
                            .unwrap_or(Style::TripleSingleQuoted)
                        };
                    self.token(&format!("{}:", quoted(key, key_style)))?;
                    self.glued = true;
                    self.value(value)?;
                }
                Ok(start..self.raw("}")?.end)
            }
        }
    }

    /// A text field on its own lines; the line goes on after its closing `;`. A line after the
    /// first that starts with `;` would end the field, so it gets a leading space: CIF has no other
    /// way to write it.
    fn text_field(&mut self, text: &str) -> Written<Range<usize>> {
        if !self.at_line_start {
            self.newline()?;
        }
        let start = self.written;
        self.put(";")?;
        for (index, line) in text.split('\n').enumerate() {
            if index > 0 {
                self.put("\n")?;
                if line.starts_with(';') {
                    self.put(" ")?;
                }
            }
            self.put(line)?;
        }
        self.put("\n;")?;
        self.at_line_start = false;
        self.glued = false;
        Ok(start..self.written)
    }
}

fn quoted(text: &str, style: Style) -> String {
    match style {
        Style::Bare | Style::Unknown | Style::Inapplicable => text.to_string(),
        Style::SingleQuoted => format!("'{text}'"),
        Style::DoubleQuoted => format!("\"{text}\""),
        Style::TripleSingleQuoted => format!("'''{text}'''"),
        Style::TripleDoubleQuoted => format!("\"\"\"{text}\"\"\""),
        Style::TextField => unreachable!("text fields take their own lines"),
    }
}
