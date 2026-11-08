//! Writing a CIF 1.1 document.
//!
//! Every value is written in its own style when that style can hold its text, else in the plainest
//! one that can. A pair is written on one line (a text field on the lines after its tag), a loop
//! with one tag per line and one row per line, every row indented, so no line starts with a bare
//! value.

use std::fmt::{self, Display, Formatter, Write};

use crate::document::{Cif, Item, Style, Value};

impl Display for Cif {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for comment in &self.comments {
            write_comment(f, comment)?;
        }
        for (index, block) in self.blocks.iter().enumerate() {
            if index > 0 || !self.comments.is_empty() {
                writeln!(f)?;
            }
            writeln!(f, "data_{}", block.name)?;
            write_items(f, &block.items)?;
        }
        Ok(())
    }
}

fn write_items(f: &mut Formatter<'_>, items: &[Item]) -> fmt::Result {
    for item in items {
        match item {
            Item::Pair { tag, value } => {
                let style = style_of(value);
                if style == Style::TextField {
                    writeln!(f, "{tag}")?;
                    write_text_field(f, &value.text)?;
                } else {
                    write!(f, "{tag} ")?;
                    write_inline(f, &value.text, style)?;
                    writeln!(f)?;
                }
            }
            Item::Loop(l) => {
                writeln!(f, "loop_")?;
                for tag in &l.tags {
                    writeln!(f, "  {tag}")?;
                }
                for row in &l.rows {
                    let mut line_open = false;
                    for value in row {
                        let style = style_of(value);
                        if style == Style::TextField {
                            if line_open {
                                writeln!(f)?;
                            }
                            write_text_field(f, &value.text)?;
                            line_open = false;
                        } else {
                            f.write_str(if line_open { " " } else { "  " })?;
                            write_inline(f, &value.text, style)?;
                            line_open = true;
                        }
                    }
                    if line_open {
                        writeln!(f)?;
                    }
                }
            }
            Item::SaveFrame { name, items } => {
                writeln!(f, "save_{name}")?;
                write_items(f, items)?;
                writeln!(f, "save_")?;
            }
            Item::Comment(text) => write_comment(f, text)?,
        }
    }
    Ok(())
}

/// The style `value` is written in: its own, or the plainest one that can hold its text.
fn style_of(value: &Value) -> Style {
    if value.style.can_hold(&value.text) {
        value.style
    } else {
        Value::new(value.text.clone()).style
    }
}

fn write_inline(f: &mut Formatter<'_>, text: &str, style: Style) -> fmt::Result {
    match style {
        Style::Bare | Style::Unknown | Style::Inapplicable => f.write_str(text),
        Style::SingleQuoted => write!(f, "'{text}'"),
        Style::DoubleQuoted => write!(f, "\"{text}\""),
        Style::TextField => unreachable!("text fields take their own lines"),
    }
}

/// A text field on its own lines. A line after the first that starts with `;` would end the field,
/// so it gets a leading space: CIF 1.1 has no other way to write it.
fn write_text_field(f: &mut Formatter<'_>, text: &str) -> fmt::Result {
    f.write_char(';')?;
    for (index, line) in text.split('\n').enumerate() {
        if index > 0 {
            f.write_char('\n')?;
            if line.starts_with(';') {
                f.write_char(' ')?;
            }
        }
        f.write_str(line)?;
    }
    f.write_str("\n;\n")
}

fn write_comment(f: &mut Formatter<'_>, text: &str) -> fmt::Result {
    for line in text.split('\n') {
        writeln!(f, "#{line}")?;
    }
    Ok(())
}
