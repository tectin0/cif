//! Any document, CIF 1.1 or 2.0, reads back as itself once written.

use cif::{read, Cif, DataBlock, Item, Loop, Style, TableEntry, Value, Version};
use proptest::prelude::*;

fn tag() -> impl Strategy<Value = String> {
    "_[a-zA-Z][a-zA-Z0-9_.\\-]{0,10}"
}

fn name() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9][a-zA-Z0-9_\\-]{0,8}"
}

fn version() -> impl Strategy<Value = Version> {
    prop_oneof![Just(Version::V1_1), Just(Version::V2_0)]
}

/// A text value in a style that can hold its text in `version`, with quotes, `#`, `;`, brackets
/// and reserved words among the texts.
fn text_value(version: Version) -> impl Strategy<Value = Value> {
    let inline = "[ a-zA-Z0-9.()+\\-;#'\"$_\\[\\]{}]{0,10}";
    let multiline = "[ a-zA-Z0-9.;#'\"\n]{0,30}";
    prop_oneof![
        (inline, Just(Style::Bare)),
        (inline, Just(Style::SingleQuoted)),
        (inline, Just(Style::DoubleQuoted)),
        (multiline, Just(Style::TripleSingleQuoted)),
        (multiline, Just(Style::TripleDoubleQuoted)),
        (multiline, Just(Style::TextField)),
        prop_oneof![
            Just("data_x".to_string()),
            Just("LOOP_".to_string()),
            Just("save_".to_string())
        ]
        .prop_map(|text| (text, Style::Bare)),
        Just(("?".to_string(), Style::Unknown)),
        Just((".".to_string(), Style::Inapplicable)),
    ]
    .prop_filter("a style the version has for the text", move |(text, style)| {
        style.can_hold(text, version)
    })
    .prop_map(|(text, style)| Value::Text { text, style })
}

/// A value of `version`: text, and in CIF 2.0 also lists and tables, nested.
fn value(version: Version) -> BoxedStrategy<Value> {
    let leaf = text_value(version).boxed();
    if version == Version::V1_1 {
        return leaf;
    }
    leaf.prop_recursive(3, 16, 4, move |inner| {
        let key = prop_oneof![
            ("[a-z ]{0,6}", Just(Style::SingleQuoted)),
            ("[a-z ]{0,6}", Just(Style::DoubleQuoted)),
            ("[a-z\n']{0,6}", Just(Style::TripleDoubleQuoted)),
        ]
        .prop_filter("a key style for the key", |(key, style)| {
            style.can_hold(key, Version::V2_0)
        });
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..4).prop_map(Value::List),
            prop::collection::vec((key, inner), 0..4).prop_map(|entries| {
                Value::Table(
                    entries
                        .into_iter()
                        .map(|((key, key_style), value)| TableEntry {
                            key,
                            key_style,
                            value,
                        })
                        .collect(),
                )
            }),
        ]
    })
    .boxed()
}

fn comment() -> impl Strategy<Value = String> {
    "[ a-zA-Z0-9#;'\"_]{0,12}"
}

fn a_loop(version: Version) -> impl Strategy<Value = Loop> {
    (1usize..4).prop_flat_map(move |width| {
        (
            prop::collection::vec(tag(), width),
            prop::collection::vec(prop::collection::vec(value(version), width), 1..4),
        )
            .prop_map(|(tags, rows)| Loop { tags, rows })
    })
}

fn item(version: Version) -> impl Strategy<Value = Item> {
    prop_oneof![
        (tag(), value(version)).prop_map(|(tag, value)| Item::Pair { tag, value }),
        a_loop(version).prop_map(Item::Loop),
        comment().prop_map(Item::Comment),
    ]
}

fn items(version: Version) -> impl Strategy<Value = Vec<Item>> {
    prop::collection::vec(
        prop_oneof![
            4 => item(version),
            1 => (name(), prop::collection::vec(item(version), 0..4))
                .prop_map(|(name, items)| Item::SaveFrame { name, items }),
        ],
        0..8,
    )
}

fn document() -> impl Strategy<Value = Cif> {
    version().prop_flat_map(|version| {
        (
            prop::collection::vec(comment(), 0..3),
            prop::collection::vec(
                (name(), items(version)).prop_map(|(name, items)| DataBlock { name, items }),
                0..4,
            ),
        )
            .prop_map(move |(comments, blocks)| Cif {
                version,
                comments,
                blocks,
            })
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn a_written_document_reads_back_as_itself(cif in document()) {
        let written = cif.to_string();
        prop_assert_eq!(read(&written), Ok(cif), "written:\n{}", written);
    }

    #[test]
    fn a_new_value_reads_back_with_its_text_in_either_version(
        text in "[ a-zA-Z0-9.;#'\"_\\[\\]{}\n]{0,20}",
        version in version(),
    ) {
        prop_assume!(Style::TextField.can_hold(&text, version));
        let cif = Cif {
            version,
            comments: Vec::new(),
            blocks: vec![DataBlock {
                name: "x".into(),
                items: vec![Item::Pair { tag: "_t".into(), value: Value::new(text.clone()) }],
            }],
        };
        let read_back = read(&cif.to_string()).unwrap();
        prop_assert_eq!(read_back.blocks[0].value("_t").and_then(Value::text), Some(text.as_str()));
    }
}
