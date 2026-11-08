//! What a CIF document holds.

/// A CIF document: its data blocks in order, and the comments before the first one.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Cif {
    /// The comments before the first data block (the `#\#CIF_1.1` line among them), without
    /// their `#`.
    pub comments: Vec<String>,
    /// The data blocks, in the order they were written.
    pub blocks: Vec<DataBlock>,
}

impl Cif {
    /// The first data block named `name`, ignoring case.
    pub fn block(&self, name: &str) -> Option<&DataBlock> {
        self.blocks
            .iter()
            .find(|block| block.name.eq_ignore_ascii_case(name))
    }
}

/// A data block: `data_<name>` and the items after it.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DataBlock {
    /// The name after `data_`, as written.
    pub name: String,
    /// The block's items, in the order they were written.
    pub items: Vec<Item>,
}

impl DataBlock {
    /// A block named `name` without items.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            items: Vec::new(),
        }
    }

    /// The value of `tag`: of its pair, or of its loop if that has a single row.
    pub fn value(&self, tag: &str) -> Option<&Value> {
        self.items.iter().find_map(|item| match item {
            Item::Pair { tag: t, value } if same_tag(t, tag) => Some(value),
            Item::Loop(l) if l.rows.len() == 1 => l.get(0, tag),
            _ => None,
        })
    }

    /// Every value of `tag`: the one of its pair, or its loop's column.
    pub fn values(&self, tag: &str) -> Option<Vec<&Value>> {
        self.items.iter().find_map(|item| match item {
            Item::Pair { tag: t, value } if same_tag(t, tag) => Some(vec![value]),
            Item::Loop(l) => l.column_values(tag),
            _ => None,
        })
    }

    /// The loop holding `tag`.
    pub fn loop_with(&self, tag: &str) -> Option<&Loop> {
        self.items.iter().find_map(|item| match item {
            Item::Loop(l) if l.column(tag).is_some() => Some(l),
            _ => None,
        })
    }

    /// Whether the block holds `tag`, as a pair or in a loop.
    pub fn contains(&self, tag: &str) -> bool {
        self.items.iter().any(|item| match item {
            Item::Pair { tag: t, .. } => same_tag(t, tag),
            Item::Loop(l) => l.column(tag).is_some(),
            _ => false,
        })
    }
}

/// What a data block or save frame holds, in the order it was written.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Item {
    /// A tag and its value.
    Pair {
        /// The tag, with its leading `_`, as written.
        tag: String,
        /// Its value.
        value: Value,
    },
    /// A loop.
    Loop(Loop),
    /// A save frame (`save_<name>` ... `save_`), as dictionaries use them.
    SaveFrame {
        /// The name after `save_`, as written.
        name: String,
        /// The frame's items.
        items: Vec<Item>,
    },
    /// A comment, without its `#`. A comment written among a loop's or a pair's values is kept
    /// after that loop or pair.
    Comment(String),
}

/// A loop: its tags, and its values row by row. CIF has no way to write a loop without rows.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Loop {
    /// The tags, as written.
    pub tags: Vec<String>,
    /// The rows, each with one value per tag.
    pub rows: Vec<Vec<Value>>,
}

impl Loop {
    /// The column of `tag`.
    pub fn column(&self, tag: &str) -> Option<usize> {
        self.tags.iter().position(|t| same_tag(t, tag))
    }

    /// The value of `tag` in row `row`.
    pub fn get(&self, row: usize, tag: &str) -> Option<&Value> {
        let column = self.column(tag)?;
        self.rows.get(row).and_then(|values| values.get(column))
    }

    /// The values of `tag`, row by row.
    pub fn column_values(&self, tag: &str) -> Option<Vec<&Value>> {
        let column = self.column(tag)?;
        Some(self.rows.iter().map(|row| &row[column]).collect())
    }
}

/// A value and the way it was written.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Value {
    /// The text, without quotes or text-field delimiters. `?` for [`Style::Unknown`] and `.` for
    /// [`Style::Inapplicable`].
    pub text: String,
    /// How it is written.
    pub style: Style,
}

impl Value {
    /// `text`, written in the plainest style that can hold it.
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let style = Style::plainest_for(&text);
        Self { text, style }
    }

    /// `?`: the value is unknown.
    pub fn unknown() -> Self {
        Self {
            text: "?".into(),
            style: Style::Unknown,
        }
    }

    /// `.`: the item doesn't apply.
    pub fn inapplicable() -> Self {
        Self {
            text: ".".into(),
            style: Style::Inapplicable,
        }
    }

    /// The text, unless the value is `?` or `.`.
    pub fn as_str(&self) -> Option<&str> {
        match self.style {
            Style::Unknown | Style::Inapplicable => None,
            _ => Some(&self.text),
        }
    }

    /// The value with `text` in its place, in the same style if that can hold `text`, else in the
    /// plainest one that can.
    pub fn with_text(&self, text: impl Into<String>) -> Self {
        let text = text.into();
        let style = if self.style.can_hold(&text) {
            self.style
        } else {
            Style::plainest_for(&text)
        };
        Self { text, style }
    }
}

/// How a value is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Style {
    /// Without quotes.
    Bare,
    /// In single quotes.
    SingleQuoted,
    /// In double quotes.
    DoubleQuoted,
    /// In a text field: between lines starting with `;`.
    TextField,
    /// `?`, unquoted.
    Unknown,
    /// `.`, unquoted.
    Inapplicable,
}

impl Style {
    /// Whether a value written in this style reads back as `text` in this style.
    pub fn can_hold(self, text: &str) -> bool {
        match self {
            Style::Bare => can_be_bare(text),
            Style::SingleQuoted => can_be_quoted(text, '\''),
            Style::DoubleQuoted => can_be_quoted(text, '"'),
            Style::TextField => !text.split('\n').skip(1).any(|line| line.starts_with(';')),
            Style::Unknown => text == "?",
            Style::Inapplicable => text == ".",
        }
    }

    /// The plainest style that can hold `text`, or a text field for a text no style can hold:
    /// CIF 1.1 can't write a line starting with `;` after the first line of a text, so the
    /// writer puts a space before each such line (see [`crate::Cif`]'s `Display`).
    fn plainest_for(text: &str) -> Style {
        [Style::Bare, Style::SingleQuoted, Style::DoubleQuoted]
            .into_iter()
            .find(|style| style.can_hold(text))
            .unwrap_or(Style::TextField)
    }
}

/// Whether two tags (or block or frame names) are the same, ignoring case.
pub fn same_tag(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// The words CIF reserves, ignoring case, by their prefix.
pub(crate) fn is_reserved(word: &str) -> bool {
    let lower = word.to_ascii_lowercase();
    lower.starts_with("data_")
        || lower.starts_with("save_")
        || lower == "loop_"
        || lower == "global_"
        || lower == "stop_"
}

/// A bare value starting with `;` is safe because the writer never starts a line with a bare value.
fn can_be_bare(text: &str) -> bool {
    !text.is_empty()
        && text != "?"
        && text != "."
        && !text.chars().any(char::is_whitespace)
        && !text.starts_with(['_', '#', '\'', '"'])
        && !is_reserved(text)
}

/// A quoted value ends at its quote followed by whitespace or the end of the line, so the text
/// can hold its own quote anywhere but before whitespace or at its end.
fn can_be_quoted(text: &str, quote: char) -> bool {
    if text.contains(['\n', '\r']) || text.ends_with(quote) {
        return false;
    }
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == quote && chars.peek().is_some_and(|next| next.is_whitespace()) {
            return false;
        }
    }
    true
}
