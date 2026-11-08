//! Any document reads back as itself once written.

use cif::{read, Cif, DataBlock, Item, Loop, Style, Value};
use proptest::prelude::*;

fn tag() -> impl Strategy<Value = String> {
    "_[a-zA-Z][a-zA-Z0-9_.\\-]{0,10}"
}

fn name() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9][a-zA-Z0-9_\\-]{0,8}"
}

/// A value in a style that can hold its text, with quotes, `#`, `;` and reserved words among the
/// texts.
fn value() -> impl Strategy<Value = Value> {
    let inline = "[ a-zA-Z0-9.()+\\-;#'\"$_\\[\\]]{0,10}";
    let multiline = "[ a-zA-Z0-9.;#'\"\n]{0,30}";
    prop_oneof![
        (inline, Just(Style::Bare)),
        (inline, Just(Style::SingleQuoted)),
        (inline, Just(Style::DoubleQuoted)),
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
    .prop_map(|(text, style)| {
        let value = Value { text, style };
        if style.can_hold(&value.text) {
            value
        } else {
            Value::new(value.text)
        }
    })
    .prop_filter("a text CIF 1.1 can write", |value| {
        value.style.can_hold(&value.text)
    })
}

fn comment() -> impl Strategy<Value = String> {
    "[ a-zA-Z0-9#;'\"_]{0,12}"
}

fn a_loop() -> impl Strategy<Value = Loop> {
    (1usize..4).prop_flat_map(|width| {
        (
            prop::collection::vec(tag(), width),
            prop::collection::vec(prop::collection::vec(value(), width), 1..4),
        )
            .prop_map(|(tags, rows)| Loop { tags, rows })
    })
}

fn item() -> impl Strategy<Value = Item> {
    prop_oneof![
        (tag(), value()).prop_map(|(tag, value)| Item::Pair { tag, value }),
        a_loop().prop_map(Item::Loop),
        comment().prop_map(Item::Comment),
    ]
}

fn items() -> impl Strategy<Value = Vec<Item>> {
    prop::collection::vec(
        prop_oneof![
            4 => item(),
            1 => (name(), prop::collection::vec(item(), 0..4))
                .prop_map(|(name, items)| Item::SaveFrame { name, items }),
        ],
        0..8,
    )
}

fn document() -> impl Strategy<Value = Cif> {
    (
        prop::collection::vec(comment(), 0..3),
        prop::collection::vec(
            (name(), items()).prop_map(|(name, items)| DataBlock { name, items }),
            0..4,
        ),
    )
        .prop_map(|(comments, blocks)| Cif { comments, blocks })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn a_written_document_reads_back_as_itself(cif in document()) {
        let written = cif.to_string();
        prop_assert_eq!(read(&written), Ok(cif), "written:\n{}", written);
    }

    #[test]
    fn a_new_value_reads_back_with_its_text(text in "[ a-zA-Z0-9.;#'\"_\n]{0,20}") {
        prop_assume!(Style::TextField.can_hold(&text));
        let value = Value::new(text.clone());
        let cif = Cif {
            comments: Vec::new(),
            blocks: vec![DataBlock {
                name: "x".into(),
                items: vec![Item::Pair { tag: "_t".into(), value }],
            }],
        };
        let read_back = read(&cif.to_string()).unwrap();
        prop_assert_eq!(read_back.blocks[0].value("_t").map(|v| v.text.clone()), Some(text));
    }
}
