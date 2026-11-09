//! What a CIF document holds.

/// The version of the CIF syntax a document is written in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "get-size", derive(get_size2::GetSize))]
pub enum Version {
    /// CIF 1.1: values are text.
    #[default]
    V1_1,
    /// CIF 2.0: values may also be lists and tables, and text may be triple-quoted. A document
    /// starts with the line `#\#CIF_2.0`.
    V2_0,
}

/// A CIF document: its version, its data blocks in order, and the comments before the first one.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "get-size", derive(get_size2::GetSize))]
pub struct Cif {
    /// The syntax the document is written in.
    pub version: Version,
    /// The comments before the first data block, without their `#`. For CIF 1.1 they include the
    /// `#\#CIF_1.1` line if the document has one; CIF 2.0's `#\#CIF_2.0` line is [`Self::version`].
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
#[cfg_attr(feature = "get-size", derive(get_size2::GetSize))]
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

    /// `text` as a block name: with `_` for each whitespace character, which a name can't hold.
    pub fn name_for(text: &str) -> String {
        text.chars()
            .map(|c| if c.is_whitespace() { '_' } else { c })
            .collect()
    }

    /// The value of `tag`'s pair.
    pub fn pair(&self, tag: &str) -> Option<&Value> {
        self.items.iter().find_map(|item| match item {
            Item::Pair { tag: t, value } if same_tag(t, tag) => Some(value),
            _ => None,
        })
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
#[cfg_attr(feature = "get-size", derive(get_size2::GetSize))]
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
    /// after that loop or pair; one inside a list or table is not kept.
    Comment(String),
}

/// A loop: its tags, and its values row by row. CIF has no way to write a loop without rows.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "get-size", derive(get_size2::GetSize))]
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

/// A value: text and the way it is written, or (CIF 2.0) a list or a table of values.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "get-size", derive(get_size2::GetSize))]
pub enum Value {
    /// Text, without quotes or delimiters: `?` for [`Style::Unknown`] and `.` for
    /// [`Style::Inapplicable`].
    Text {
        /// The text.
        text: String,
        /// How it is written.
        style: Style,
    },
    /// A list, `[a b c]` (CIF 2.0).
    List(Vec<Value>),
    /// A table, `{'key':value ...}` (CIF 2.0), its entries in order.
    Table(Vec<TableEntry>),
}

/// An entry of a [`Value::Table`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "get-size", derive(get_size2::GetSize))]
pub struct TableEntry {
    /// The key.
    pub key: String,
    /// How the key is written: one of the quoted styles.
    pub key_style: Style,
    /// The value.
    pub value: Value,
}

impl Value {
    /// `text`, written in the plainest style that can hold it in both CIF versions.
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let style = Style::plainest_for(&text);
        Value::Text { text, style }
    }

    /// `?`: the value is unknown.
    pub fn unknown() -> Self {
        Value::Text {
            text: "?".into(),
            style: Style::Unknown,
        }
    }

    /// `.`: the item doesn't apply.
    pub fn inapplicable() -> Self {
        Value::Text {
            text: ".".into(),
            style: Style::Inapplicable,
        }
    }

    /// The text, unless the value is `?`, `.`, a list or a table.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Text {
                style: Style::Unknown | Style::Inapplicable,
                ..
            } => None,
            Value::Text { text, .. } => Some(text),
            Value::List(_) | Value::Table(_) => None,
        }
    }

    /// The text as written, `?` and `.` included, unless the value is a list or a table.
    pub fn text(&self) -> Option<&str> {
        match self {
            Value::Text { text, .. } => Some(text),
            Value::List(_) | Value::Table(_) => None,
        }
    }

    /// A text value with `text` in this value's place: in this value's style if that can hold
    /// `text` in both CIF versions, else in the plainest one that can. An empty text's style is
    /// the only one that can hold it, not a choice, so `text` takes the plainest style in its place.
    pub fn with_text(&self, text: impl Into<String>) -> Self {
        let text = text.into();
        match self {
            Value::Text { text: old, style }
                if !old.is_empty()
                    && style.can_hold(&text, Version::V1_1)
                    && style.can_hold(&text, Version::V2_0) =>
            {
                Value::Text {
                    text,
                    style: *style,
                }
            }
            _ => Value::new(text),
        }
    }

    /// Whether the value is a list or a table, which only CIF 2.0 can hold.
    pub fn is_compound(&self) -> bool {
        matches!(self, Value::List(_) | Value::Table(_))
    }

    /// The number the value holds, with its standard uncertainty (see [`crate::number::parse`]),
    /// or `None` for `?`, `.` or text that isn't a number.
    pub fn number(&self) -> Option<(f64, Option<f64>)> {
        crate::number::parse(self.as_str()?)
    }

    /// The number `value`, with the standard uncertainty `esd`, in this value's place: this value
    /// as it is written if it already holds that number (up to rounding), else the number written
    /// (see [`crate::number::format`]) in this value's style where that can hold it.
    pub fn with_number(&self, value: f64, esd: Option<f64>) -> Self {
        let close = |a: f64, b: f64| (a - b).abs() <= 1e-12 * a.abs().max(b.abs()).max(1.0);
        let holds = self.number().is_some_and(|(read, read_esd)| {
            close(read, value)
                && match (read_esd, esd) {
                    (Some(a), Some(b)) => close(a, b),
                    (None, None) => true,
                    _ => false,
                }
        });

        if holds {
            self.clone()
        } else {
            self.with_text(crate::number::format(value, esd))
        }
    }

    /// The value of an item that has none: this value if it is `?` or `.`, else `?`.
    pub fn emptied(&self) -> Self {
        match self {
            Value::Text {
                style: Style::Unknown | Style::Inapplicable,
                ..
            } => self.clone(),
            _ => Value::unknown(),
        }
    }
}

/// How a text value is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "get-size", derive(get_size2::GetSize))]
pub enum Style {
    /// Without quotes.
    Bare,
    /// In single quotes.
    SingleQuoted,
    /// In double quotes.
    DoubleQuoted,
    /// In triple single quotes, `'''...'''`, which may span lines (CIF 2.0).
    TripleSingleQuoted,
    /// In triple double quotes, `"""..."""`, which may span lines (CIF 2.0).
    TripleDoubleQuoted,
    /// In a text field: between lines starting with `;`.
    TextField,
    /// `?`, unquoted.
    Unknown,
    /// `.`, unquoted.
    Inapplicable,
}

impl Style {
    /// Whether a value written in this style in a document of `version` reads back as `text` in
    /// this style.
    pub fn can_hold(self, text: &str, version: Version) -> bool {
        let v2 = version == Version::V2_0;
        match self {
            Style::Bare => can_be_bare(text, version),
            Style::SingleQuoted => can_be_quoted(text, '\'', version),
            Style::DoubleQuoted => can_be_quoted(text, '"', version),
            Style::TripleSingleQuoted => v2 && can_be_triple_quoted(text, '\''),
            Style::TripleDoubleQuoted => v2 && can_be_triple_quoted(text, '"'),
            Style::TextField => !text.split('\n').skip(1).any(|line| line.starts_with(';')),
            Style::Unknown => text == "?",
            Style::Inapplicable => text == ".",
        }
    }

    /// The plainest style that can hold `text` in both CIF versions, or a text field for a text
    /// no such style can hold: CIF can't write a line starting with `;` after the first line of a
    /// text field, so the writer puts a space before each such line.
    pub(crate) fn plainest_for(text: &str) -> Style {
        [Style::Bare, Style::SingleQuoted, Style::DoubleQuoted]
            .into_iter()
            .find(|style| style.can_hold(text, Version::V1_1) && style.can_hold(text, Version::V2_0))
            .unwrap_or(Style::TextField)
    }

    /// The plainest style that can hold `text` in a document of `version`, or a text field (see
    /// [`Self::plainest_for`]).
    pub(crate) fn plainest_in(text: &str, version: Version) -> Style {
        [
            Style::Bare,
            Style::SingleQuoted,
            Style::DoubleQuoted,
            Style::TripleSingleQuoted,
            Style::TripleDoubleQuoted,
        ]
        .into_iter()
        .find(|style| style.can_hold(text, version))
        .unwrap_or(Style::TextField)
    }

    /// Whether the style quotes its text (a table's key must be quoted).
    pub fn is_quoted(self) -> bool {
        matches!(
            self,
            Style::SingleQuoted
                | Style::DoubleQuoted
                | Style::TripleSingleQuoted
                | Style::TripleDoubleQuoted
        )
    }
}

/// Whether two tags are the same, ignoring case, with DDLm's `.` between category and attribute
/// read as `_` (`_atom_site.label` is `_atom_site_label`).
pub fn same_tag(a: &str, b: &str) -> bool {
    let fold = |c: char| match c {
        '.' => '_',
        c => c.to_ascii_lowercase(),
    };
    a.len() == b.len() && a.chars().map(fold).eq(b.chars().map(fold))
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

/// The characters that end a bare value in CIF 2.0, besides whitespace.
pub(crate) const V2_DELIMITERS: [char; 4] = ['[', ']', '{', '}'];

/// A bare value starting with `;` is safe because the writer never starts a line with a bare value.
fn can_be_bare(text: &str, version: Version) -> bool {
    !text.is_empty()
        && text != "?"
        && text != "."
        && !text.chars().any(char::is_whitespace)
        && !text.starts_with(['_', '#', '\'', '"'])
        && !is_reserved(text)
        && (version == Version::V1_1 || !text.contains(V2_DELIMITERS))
}

/// In CIF 1.1 a quoted value ends at its quote followed by whitespace or the end of the line, so
/// the text can hold its own quote anywhere but before whitespace or at its end. In CIF 2.0 a
/// quoted value ends at its next quote, so the text can't hold it at all.
fn can_be_quoted(text: &str, quote: char, version: Version) -> bool {
    if text.contains(['\n', '\r']) {
        return false;
    }
    if version == Version::V2_0 {
        return !text.contains(quote);
    }
    if text.ends_with(quote) {
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

/// A triple-quoted value ends at the next three quotes, so the text can't hold three in a row, nor
/// end with one (which would join the closing three).
fn can_be_triple_quoted(text: &str, quote: char) -> bool {
    let delimiter: String = std::iter::repeat_n(quote, 3).collect();
    !text.contains(&delimiter) && !text.ends_with(quote)
}
