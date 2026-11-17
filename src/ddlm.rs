//! DDLm dictionaries: what their categories and items are, read from the dictionary's own CIF.
//!
//! A [`Dictionary`] is read from a dictionary document, such as the IUCr core dictionary
//! (`cif_core.dic`), with the documents its definitions import (`_import.get`, such as
//! `templ_attr.cif`). With the `core-dictionary` feature, [`core`] is the core dictionary, as
//! published by COMCIFS under CC BY 4.0 (see `dictionaries/README.md`).

use std::collections::{HashMap, HashSet};

use crate::document::{Cif, Item, Value};

/// Why a document isn't a dictionary this module can read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictionaryError {
    /// What is wrong, naming the definition at fault.
    pub message: String,
}

impl std::fmt::Display for DictionaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for DictionaryError {}

fn error<T>(message: impl Into<String>) -> Result<T, DictionaryError> {
    Err(DictionaryError {
        message: message.into(),
    })
}

/// A DDLm dictionary: its categories and item definitions, in the order it defines them.
#[derive(Debug, Clone, Default)]
pub struct Dictionary {
    /// `_dictionary.title`.
    pub title: String,
    /// `_dictionary.version`.
    pub version: String,
    categories: Vec<Category>,
    definitions: Vec<Definition>,
    /// The definition of each name and alias, lowercase.
    by_name: HashMap<String, usize>,
    /// The definition of each name and alias as [`normalised`] writes it; `None` where two
    /// definitions share it.
    by_normalised: HashMap<String, Option<usize>>,
    /// The category of each name, lowercase.
    by_category: HashMap<String, usize>,
}

/// A category: a set of items given once per data block, or a loop of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Category {
    /// `_definition.id`, as written (`ATOM_SITE`).
    pub name: String,
    pub class: CategoryClass,
    /// The tags whose values tell the category's rows apart (`_category_key.name`).
    pub keys: Vec<String>,
    /// The category it belongs to (`_name.category_id`).
    pub parent: String,
    pub description: Option<String>,
}

/// A category's `_definition.class`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CategoryClass {
    /// The dictionary's top category.
    Head,
    /// Items given at most once per data block.
    Set,
    /// Items given in a loop, one row each.
    Loop,
    /// Functions of dREL methods, which documents don't hold.
    Functions,
}

/// The definition of an item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
    /// `_definition.id`, as written (`_atom_site.ADP_type`).
    pub name: String,
    /// The other names the item may be written under, older ones among them.
    pub aliases: Vec<Alias>,
    /// The category it belongs to (`_name.category_id`), as written.
    pub category: String,
    /// Its name within the category (`_name.object_id`).
    pub object: String,
    /// `_type.purpose`: `Measurand`, `SU` (the standard uncertainty of another item), `Encode`, ...
    pub purpose: Option<String>,
    /// `_type.contents`: `Real`, `Integer`, `Text`, `Code`, `Word`, ...
    pub contents: Option<String>,
    /// `_type.container`: `Single`, `List`, `Matrix`, ...
    pub container: Option<String>,
    /// `_units.code` (`angstroms`, `none`).
    pub units: Option<String>,
    /// `_description.common`: the name the item is commonly known by, if the dictionary gives one.
    pub common_name: Option<String>,
    pub description: Option<String>,
    /// The values the item may take, if the dictionary enumerates them.
    pub states: Vec<State>,
    /// `_enumeration.default`.
    pub default: Option<String>,
}

/// Another name of an item (`_alias.definition_id`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alias {
    pub name: String,
    /// Whether the dictionary gives a date it was deprecated on.
    pub deprecated: bool,
}

/// A value an item may take (`_enumeration_set.state`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub value: String,
    pub detail: Option<String>,
}

/// `tag` lowercase, with DDLm's `.` between category and attribute read as `_`, so a DDLm name and
/// its DDL1 form are one (`_atom_site.label` is `_atom_site_label`).
fn normalised(tag: &str) -> String {
    tag.chars()
        .map(|c| match c {
            '.' => '_',
            c => c.to_ascii_lowercase(),
        })
        .collect()
}

impl Dictionary {
    /// Reads the dictionary `document`, whose first data block holds the definitions as save
    /// frames. `import` gives the document a definition's `_import.get` names by its `file`.
    pub fn read<'a>(
        document: &'a Cif,
        import: impl Fn(&str) -> Option<&'a Cif>,
    ) -> Result<Self, DictionaryError> {
        let Some(block) = document.blocks.first() else {
            return error("the dictionary has no data block");
        };

        let mut dictionary = Dictionary {
            title: text_of(&[&block.items], "_dictionary.title").unwrap_or_default(),
            version: text_of(&[&block.items], "_dictionary.version").unwrap_or_default(),
            ..Self::default()
        };

        for item in &block.items {
            let Item::SaveFrame { name, items } = item else {
                continue;
            };

            let mut sources = vec![items.as_slice()];
            let mut visited = HashSet::new();
            imports(items, &import, &mut sources, &mut visited).map_err(|e| DictionaryError {
                message: format!("save_{name}: {}", e.message),
            })?;

            let scope = text_of(&sources, "_definition.scope");
            if scope
                .as_deref()
                .is_some_and(|s| s.eq_ignore_ascii_case("category"))
            {
                dictionary.add_category(name, &sources)?;
            } else {
                dictionary.add_definition(name, &sources)?;
            }
        }

        Ok(dictionary)
    }

    fn add_category(&mut self, frame: &str, sources: &[&[Item]]) -> Result<(), DictionaryError> {
        let name = text_of(sources, "_definition.id").unwrap_or_else(|| frame.to_string());
        let class = match text_of(sources, "_definition.class") {
            Some(class) if class.eq_ignore_ascii_case("head") => CategoryClass::Head,
            Some(class) if class.eq_ignore_ascii_case("set") => CategoryClass::Set,
            Some(class) if class.eq_ignore_ascii_case("loop") => CategoryClass::Loop,
            Some(class) if class.eq_ignore_ascii_case("functions") => CategoryClass::Functions,
            Some(class) => return error(format!("category {name}: unknown class `{class}`")),
            None => CategoryClass::Set,
        };

        self.by_category
            .insert(name.to_ascii_lowercase(), self.categories.len());
        self.categories.push(Category {
            keys: texts_of(sources, "_category_key.name"),
            parent: text_of(sources, "_name.category_id").unwrap_or_default(),
            description: text_of(sources, "_description.text").map(|text| dedent(&text)),
            name,
            class,
        });
        Ok(())
    }

    fn add_definition(&mut self, frame: &str, sources: &[&[Item]]) -> Result<(), DictionaryError> {
        let name = text_of(sources, "_definition.id").unwrap_or_else(|| frame.to_string());
        let Some(category) = text_of(sources, "_name.category_id") else {
            return error(format!("{name}: no `_name.category_id`"));
        };

        let alias_names = texts_of(sources, "_alias.definition_id");
        let deprecations = attribute(sources, "_alias.deprecation_date");
        let aliases = alias_names
            .into_iter()
            .enumerate()
            .map(|(i, alias)| Alias {
                name: alias,
                deprecated: deprecations
                    .as_ref()
                    .and_then(|dates| dates.get(i))
                    .is_some_and(|date| date.as_str().is_some()),
            })
            .collect();

        let state_values = texts_of(sources, "_enumeration_set.state");
        let details = attribute(sources, "_enumeration_set.detail");
        let states = state_values
            .into_iter()
            .enumerate()
            .map(|(i, value)| State {
                value,
                detail: details
                    .as_ref()
                    .and_then(|details| details.get(i))
                    .and_then(|detail| detail.as_str())
                    .map(dedent),
            })
            .collect();

        let index = self.definitions.len();
        let definition = Definition {
            aliases,
            category,
            object: text_of(sources, "_name.object_id").unwrap_or_default(),
            purpose: text_of(sources, "_type.purpose"),
            contents: text_of(sources, "_type.contents"),
            container: text_of(sources, "_type.container"),
            units: text_of(sources, "_units.code"),
            common_name: text_of(sources, "_description.common"),
            description: text_of(sources, "_description.text").map(|text| dedent(&text)),
            states,
            default: text_of(sources, "_enumeration.default"),
            name,
        };

        let names = std::iter::once(definition.name.as_str())
            .chain(definition.aliases.iter().map(|alias| alias.name.as_str()));
        for tag in names {
            self.by_name.insert(tag.to_ascii_lowercase(), index);
            self.by_normalised
                .entry(normalised(tag))
                .and_modify(|shared| {
                    if *shared != Some(index) {
                        *shared = None;
                    }
                })
                .or_insert(Some(index));
        }

        self.definitions.push(definition);
        Ok(())
    }

    /// The definition of `tag`: the item named so or with an alias so, ignoring case, and else
    /// the one whose name or alias is `tag` with `.` read as `_` (`_atom_site.label` and
    /// `_atom_site_label` are one), unless two items' names are (see [`Self::ambiguous_names`]).
    pub fn definition(&self, tag: &str) -> Option<&Definition> {
        let index = match self.by_name.get(&tag.to_ascii_lowercase()) {
            Some(index) => Some(*index),
            None => self.by_normalised.get(&normalised(tag)).copied().flatten(),
        };
        index.map(|index| &self.definitions[index])
    }

    /// The names that, with `.` read as `_`, are the same for two items, so [`Self::definition`]
    /// finds either only by its exact name.
    pub fn ambiguous_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self
            .by_normalised
            .iter()
            .filter(|(_, index)| index.is_none())
            .map(|(name, _)| name.as_str())
            .collect();
        names.sort_unstable();
        names
    }

    /// The category named `name`, ignoring case.
    pub fn category(&self, name: &str) -> Option<&Category> {
        self.by_category
            .get(&name.to_ascii_lowercase())
            .map(|index| &self.categories[*index])
    }

    pub fn categories(&self) -> &[Category] {
        &self.categories
    }

    pub fn definitions(&self) -> &[Definition] {
        &self.definitions
    }

    /// The items of the category `name`, ignoring case.
    pub fn items_of<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Definition> + 'a {
        self.definitions
            .iter()
            .filter(move |definition| definition.category.eq_ignore_ascii_case(name))
    }
}

impl Definition {
    /// The name to write the item under in a document of `version`: in CIF 2.0 its DDLm name, in
    /// CIF 1.1 its first alias that isn't deprecated and has no `.` (its DDL1 name), if it has one.
    pub fn tag_for(&self, version: crate::Version) -> &str {
        match version {
            crate::Version::V2_0 => &self.name,
            crate::Version::V1_1 => self
                .aliases
                .iter()
                .find(|alias| !alias.deprecated && !alias.name.contains('.'))
                .map_or(&self.name, |alias| &alias.name),
        }
    }

    /// The item's name for people: its [`Self::common_name`], else its category and object ids as
    /// [`title`] writes them (`Cell angle beta`, `Atom site fractional x`).
    pub fn title(&self) -> String {
        match &self.common_name {
            Some(name) => name.clone(),
            None => title(&format!("{}_{}", self.category, self.object)),
        }
    }

    /// Whether the item is the standard uncertainty of another, which CIF writes in parentheses
    /// after that item's value instead.
    pub fn is_uncertainty(&self) -> bool {
        self.purpose
            .as_deref()
            .is_some_and(|purpose| purpose.eq_ignore_ascii_case("su"))
    }
}

/// The abbreviations of the core dictionary's category and object ids, each with the words it
/// stands for.
const ABBREVIATIONS: &[(&str, &str)] = &[
    ("abbrev", "abbreviation"),
    ("abs", "absolute"),
    ("absorpt", "absorption"),
    ("ang", "angle"),
    ("aniso", "anisotropic"),
    ("av", "average"),
    ("backgd", "background"),
    ("bg", "background"),
    ("calc", "calculated"),
    ("cartn", "Cartesian"),
    ("coef", "coefficient"),
    ("coefs", "coefficients"),
    ("conn", "connectivity"),
    ("coords", "coordinates"),
    ("defn", "definition"),
    ("depnum", "deposition number"),
    ("detect", "detector"),
    ("dict", "dictionary"),
    ("diff", "difference"),
    ("diffr", "diffractometer"),
    ("diffrn", "diffraction"),
    ("dist", "distance"),
    ("dtime", "dead time"),
    ("equiv", "equivalent"),
    ("exptl", "experimental"),
    ("fom", "figure of merit"),
    ("fract", "fractional"),
    ("geom", "geometry"),
    ("gt", "greater than"),
    ("hbond", "hydrogen bond"),
    ("hi", "high"),
    ("horiz", "horizontal"),
    ("imag", "imaginary"),
    ("incl", "included"),
    ("incr", "increment"),
    ("info", "information"),
    ("inv", "inverse"),
    ("iso", "isotropic"),
    ("ls", "least-squares"),
    ("lt", "less than"),
    ("mat", "matrix"),
    ("meas", "measured"),
    ("mid", "middle"),
    ("orient", "orientation"),
    ("param", "parameter"),
    ("perp", "perpendicular"),
    ("polarisn", "polarisation"),
    ("posn", "position"),
    ("prep", "preparation"),
    ("publ", "publication"),
    ("rad", "radius"),
    ("recd", "received"),
    ("ref", "reference"),
    ("refln", "reflection"),
    ("reflns", "reflections"),
    ("res", "resolution"),
    ("resol", "resolution"),
    ("scat", "scattering"),
    ("su", "standard uncertainty"),
    ("suppl", "supplementary"),
    ("symop", "symmetry operation"),
    ("techeditor", "technical editor"),
    ("temp", "temperature"),
    ("transf", "transformation"),
    ("vec", "vector"),
    ("vert", "vertical"),
    ("wt", "weight"),
    ("xray", "X-ray"),
];

/// `id`, a category or object id as items write it (`atom_site_fract_x`), as a name for people:
/// its words apart, [`ABBREVIATIONS`] written out, the first letter capitalised and the others as
/// written, so symbols keep their case (`Atom site fractional x`, `Cell formula units Z`).
pub fn title(id: &str) -> String {
    let words: Vec<&str> = id
        .split('_')
        .filter(|word| !word.is_empty())
        .map(|word| {
            ABBREVIATIONS
                .iter()
                .find(|(abbreviation, _)| abbreviation.eq_ignore_ascii_case(word))
                .map_or(word, |(_, words)| *words)
        })
        .collect();
    let title = words.join(" ");

    let mut chars = title.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => title,
    }
}

/// Adds to `sources` the save frames `items` imports (`_import.get`), each followed by what it
/// imports itself, in Contents mode: a frame's own attributes come before those it imports.
fn imports<'a>(
    items: &[Item],
    import: &impl Fn(&str) -> Option<&'a Cif>,
    sources: &mut Vec<&'a [Item]>,
    visited: &mut HashSet<(String, String)>,
) -> Result<(), DictionaryError> {
    let Some(value) = attribute(&[items], "_import.get").and_then(|values| values.first().copied())
    else {
        return Ok(());
    };
    let Value::List(entries) = value else {
        return error("`_import.get` isn't a list");
    };

    for entry in entries {
        let Value::Table(table) = entry else {
            return error("an `_import.get` entry isn't a table");
        };
        let key = |key: &str| {
            table
                .iter()
                .find(|entry| entry.key.eq_ignore_ascii_case(key))
                .and_then(|entry| entry.value.as_str())
        };

        if let Some(mode) = key("mode").filter(|mode| !mode.eq_ignore_ascii_case("contents")) {
            return error(format!(
                "an import in `{mode}` mode, of which only Contents is read"
            ));
        }
        let (Some(file), Some(save)) = (key("file"), key("save")) else {
            return error("an `_import.get` entry without its `file` or `save`");
        };

        if !visited.insert((file.to_ascii_lowercase(), save.to_ascii_lowercase())) {
            continue;
        }
        let Some(document) = import(file) else {
            return error(format!("`{file}`, which it imports, isn't given"));
        };
        let Some(frame) = document
            .blocks
            .iter()
            .find_map(|block| save_frame(&block.items, save))
        else {
            return error(format!("`{file}` has no save frame `{save}`"));
        };

        sources.push(frame);
        imports(frame, import, sources, visited)?;
    }

    Ok(())
}

/// The items of the save frame `name` among `items`, ignoring case.
fn save_frame<'a>(items: &'a [Item], name: &str) -> Option<&'a [Item]> {
    items.iter().find_map(|item| match item {
        Item::SaveFrame { name: n, items } if n.eq_ignore_ascii_case(name) => {
            Some(items.as_slice())
        }
        _ => None,
    })
}

/// The values of the attribute `tag` in the first of `sources` that gives it: its pair's value,
/// or its loop's column.
fn attribute<'a>(sources: &[&'a [Item]], tag: &str) -> Option<Vec<&'a Value>> {
    sources.iter().find_map(|items| {
        items.iter().find_map(|item| match item {
            Item::Pair { tag: t, value } if t.eq_ignore_ascii_case(tag) => Some(vec![value]),
            Item::Loop(l) => l
                .tags
                .iter()
                .position(|t| t.eq_ignore_ascii_case(tag))
                .map(|column| l.rows.iter().map(|row| &row[column]).collect()),
            _ => None,
        })
    })
}

/// The texts of the attribute `tag`, leaving out `?` and `.`.
fn texts_of(sources: &[&[Item]], tag: &str) -> Vec<String> {
    attribute(sources, tag)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| value.as_str().map(str::to_string))
        .collect()
}

/// The text of the attribute `tag`, unless it is `?` or `.`.
fn text_of(sources: &[&[Item]], tag: &str) -> Option<String> {
    texts_of(sources, tag).into_iter().next()
}

/// `text` without the indentation its lines share, and without leading and trailing blank lines.
fn dedent(text: &str) -> String {
    let indent = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or(0);

    text.lines()
        .map(|line| line.get(indent..).unwrap_or("").trim_end())
        .collect::<Vec<_>>()
        .join("\n")
        .trim_matches('\n')
        .to_string()
}

/// The IUCr core dictionary (`cif_core.dic`), with the attribute and enumeration templates it
/// imports, read once on first use.
#[cfg(feature = "core-dictionary")]
pub fn core() -> &'static Dictionary {
    static CORE: std::sync::OnceLock<Dictionary> = std::sync::OnceLock::new();

    CORE.get_or_init(|| {
        let embedded = |name: &str, text: &str| {
            crate::read(text).unwrap_or_else(|e| panic!("the embedded `{name}` reads: {e}"))
        };
        let core = embedded("cif_core.dic", include_str!("../dictionaries/cif_core.dic"));
        let attributes = embedded(
            "templ_attr.cif",
            include_str!("../dictionaries/templ_attr.cif"),
        );
        let enumerations = embedded(
            "templ_enum.cif",
            include_str!("../dictionaries/templ_enum.cif"),
        );

        Dictionary::read(&core, |file| match file {
            "templ_attr.cif" => Some(&attributes),
            "templ_enum.cif" => Some(&enumerations),
            _ => None,
        })
        .unwrap_or_else(|e| panic!("the embedded core dictionary reads: {e}"))
    })
}

#[cfg(all(test, feature = "core-dictionary"))]
mod tests {
    use super::*;

    #[test]
    fn every_abbreviation_is_a_word_of_a_core_id() {
        let words: HashSet<String> = core()
            .definitions()
            .iter()
            .flat_map(|definition| {
                definition
                    .category
                    .split('_')
                    .chain(definition.object.split('_'))
            })
            .map(str::to_ascii_lowercase)
            .collect();

        let unused: Vec<&str> = ABBREVIATIONS
            .iter()
            .map(|(abbreviation, _)| *abbreviation)
            .filter(|abbreviation| !words.contains(*abbreviation))
            .collect();
        assert!(unused.is_empty(), "not in any core id: {unused:?}");
    }
}
