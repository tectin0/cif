//! Writing a CIF 1.1 or 2.0 document.
//!
//! A document is written in its version, except that a CIF 1.1 document holding a list or table is
//! written as CIF 2.0, the only version that can hold one. Every text value is written in its own
//! style when that style can hold its text in the written version, else in the plainest one that
//! can. A pair is written on one line (a text field on the lines after its tag), a loop with one
//! tag per line and one row per line, every row indented, so no line starts with a bare value.

use std::fmt::{self, Display, Formatter, Write};

use crate::document::{Cif, Item, Style, TableEntry, Value, Version};
use crate::read::V2_MAGIC;

impl Display for Cif {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let version = if self.version == Version::V1_1 && self.blocks.iter().any(holds_compound) {
            Version::V2_0
        } else {
            self.version
        };
        let mut out = Out {
            f,
            version,
            at_line_start: true,
            glued: false,
        };

        if version == Version::V2_0 {
            out.raw(V2_MAGIC)?;
            out.newline()?;
        }
        for comment in &self.comments {
            out.comment(comment)?;
        }
        for (index, block) in self.blocks.iter().enumerate() {
            if index > 0 || !self.comments.is_empty() {
                out.newline()?;
            }
            out.raw(&format!("data_{}", block.name))?;
            out.newline()?;
            out.items(&block.items)?;
        }
        Ok(())
    }
}

impl Display for Value {
    /// The value as CIF 2.0 writes it.
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut out = Out {
            f,
            version: Version::V2_0,
            at_line_start: false,
            glued: true,
        };
        out.value(self)
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

/// The written text and where it stands: at the start of a line, or right after a token that the
/// next one is glued to (an opening bracket or brace, a table key's colon).
struct Out<'a, 'b> {
    f: &'a mut Formatter<'b>,
    version: Version,
    at_line_start: bool,
    glued: bool,
}

impl Out<'_, '_> {
    /// Writes `text` as it is, where the line stands.
    fn raw(&mut self, text: &str) -> fmt::Result {
        self.f.write_str(text)?;
        self.at_line_start = false;
        self.glued = false;
        Ok(())
    }

    fn newline(&mut self) -> fmt::Result {
        self.f.write_char('\n')?;
        self.at_line_start = true;
        self.glued = false;
        Ok(())
    }

    /// Writes `token` after a space, unless it starts the line or follows a glued token.
    fn token(&mut self, token: &str) -> fmt::Result {
        if !self.at_line_start && !self.glued {
            self.f.write_char(' ')?;
        }
        self.raw(token)
    }

    fn comment(&mut self, text: &str) -> fmt::Result {
        if !self.at_line_start {
            self.newline()?;
        }
        for line in text.split('\n') {
            self.raw(&format!("#{line}"))?;
            self.newline()?;
        }
        Ok(())
    }

    fn items(&mut self, items: &[Item]) -> fmt::Result {
        for item in items {
            match item {
                Item::Pair { tag, value } => {
                    self.raw(tag)?;
                    self.value(value)?;
                    self.newline()?;
                }
                Item::Loop(l) => {
                    self.raw("loop_")?;
                    self.newline()?;
                    for tag in &l.tags {
                        self.raw(&format!("  {tag}"))?;
                        self.newline()?;
                    }
                    for row in &l.rows {
                        self.raw(" ")?;
                        for value in row {
                            self.value(value)?;
                        }
                        self.newline()?;
                    }
                }
                Item::SaveFrame { name, items } => {
                    self.raw(&format!("save_{name}"))?;
                    self.newline()?;
                    self.items(items)?;
                    self.raw("save_")?;
                    self.newline()?;
                }
                Item::Comment(text) => self.comment(text)?,
            }
        }
        Ok(())
    }

    /// The style `text` is written in: `style`, or the plainest one that can hold it.
    fn style_for(&self, text: &str, style: Style) -> Style {
        if style.can_hold(text, self.version) {
            style
        } else {
            Style::plainest_in(text, self.version)
        }
    }

    fn value(&mut self, value: &Value) -> fmt::Result {
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
                self.token("[")?;
                self.glued = true;
                for value in values {
                    self.value(value)?;
                }
                self.raw("]")
            }
            Value::Table(entries) => {
                self.token("{")?;
                self.glued = true;
                for TableEntry {
                    key,
                    key_style,
                    value,
                } in entries
                {
                    let key_style = if key_style.is_quoted() && key_style.can_hold(key, self.version)
                    {
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
                self.raw("}")
            }
        }
    }

    /// A text field on its own lines; the line goes on after its closing `;`. A line after the
    /// first that starts with `;` would end the field, so it gets a leading space: CIF has no other
    /// way to write it.
    fn text_field(&mut self, text: &str) -> fmt::Result {
        if !self.at_line_start {
            self.newline()?;
        }
        self.f.write_char(';')?;
        for (index, line) in text.split('\n').enumerate() {
            if index > 0 {
                self.f.write_char('\n')?;
                if line.starts_with(';') {
                    self.f.write_char(' ')?;
                }
            }
            self.f.write_str(line)?;
        }
        self.f.write_str("\n;")?;
        self.at_line_start = false;
        self.glued = false;
        Ok(())
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
