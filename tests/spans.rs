//! Where a document's parts are in its text, and writing one item or value on its own.

use std::ops::Range;

use cif::{read, read_value, read_with_spans, Cif, Item, ItemSpans, Spans, Value, Version};

/// Checks that every range of `spans` holds what `cif` has there in `text`.
fn assert_spans_hold_the_document(text: &str, cif: &Cif, spans: &Spans) {
    assert_eq!(cif.blocks.len(), spans.blocks.len());
    for (comment, range) in cif.comments.iter().zip(&spans.comments) {
        assert_eq!(
            text[range.clone()].trim_end_matches('\r'),
            format!("#{comment}")
        );
    }
    for (block, block_spans) in cif.blocks.iter().zip(&spans.blocks) {
        assert_eq!(block.comments.len(), block_spans.comments.len());
        for (comment, range) in block.comments.iter().zip(&block_spans.comments) {
            assert_eq!(
                text[range.clone()].trim_end_matches('\r'),
                format!("#{comment}")
            );
        }
        assert!(
            text[block_spans.header.clone()].eq_ignore_ascii_case(&format!("data_{}", block.name))
        );
        assert_items(text, cif.version, &block.items, &block_spans.items);
    }
}

fn assert_items(text: &str, version: Version, items: &[Item], spans: &[ItemSpans]) {
    assert_eq!(items.len(), spans.len());
    for (item, spans) in items.iter().zip(spans) {
        let value_at = |range: &Range<usize>, value: &Value| {
            assert_eq!(
                &read_value(&text[range.clone()], version).unwrap(),
                value,
                "{:?}",
                &text[range.clone()]
            );
        };
        match (item, spans) {
            (Item::Pair { tag, value }, ItemSpans::Pair { tag: t, value: v }) => {
                assert_eq!(&text[t.clone()], tag);
                value_at(v, value);
            }
            (
                Item::Loop(l),
                ItemSpans::Loop {
                    keyword,
                    tags,
                    values,
                },
            ) => {
                assert!(text[keyword.clone()].eq_ignore_ascii_case("loop_"));
                for (tag, range) in l.tags.iter().zip(tags) {
                    assert_eq!(&text[range.clone()], tag);
                }
                assert_eq!(values.len(), l.tags.len() * l.rows.len());
                for (value, range) in l.rows.iter().flatten().zip(values) {
                    value_at(range, value);
                }
            }
            (
                Item::SaveFrame { name, items },
                ItemSpans::SaveFrame {
                    header,
                    items: frame,
                    end,
                },
            ) => {
                assert!(text[header.clone()].eq_ignore_ascii_case(&format!("save_{name}")));
                assert!(text[end.clone()].eq_ignore_ascii_case("save_"));
                assert_items(text, version, items, frame);
            }
            (Item::Comment(comment), ItemSpans::Comment(range)) => {
                assert_eq!(
                    text[range.clone()].trim_end_matches('\r'),
                    format!("#{comment}")
                );
            }
            other => panic!("an item and the ranges of another kind: {other:?}"),
        }
    }
}

const MIXED: &str = "# before
data_x
_a 1.5(2) # after a
_b 'two words'
_c
;line one
line two
;
loop_
  _d
  # among the tags
  _e
  1 'x y'
  # among the values
  2 ?
save_frame
_f .
save_
";

#[test]
fn every_part_of_a_document_is_where_its_spans_say() {
    let (cif, spans) = read_with_spans(MIXED).unwrap();
    assert_spans_hold_the_document(MIXED, &cif, &spans);

    let ItemSpans::Pair { value, .. } = &spans.blocks[0].items[2] else {
        panic!("a pair")
    };
    assert_eq!(&MIXED[value.clone()], "'two words'");
    let ItemSpans::Pair { value, .. } = &spans.blocks[0].items[3] else {
        panic!("the text field")
    };
    assert_eq!(&MIXED[value.clone()], ";line one\nline two\n;");
}

#[test]
fn spans_count_from_the_start_of_the_text_given() {
    let text = "\u{feff}#\\#CIF_2.0\ndata_x\n_a [1 2]\n";
    let (cif, spans) = read_with_spans(text).unwrap();

    assert_eq!(&text[spans.blocks[0].header.clone()], "data_x");
    assert_spans_hold_the_document(text, &cif, &spans);
}

#[test]
fn every_value_of_every_test_file_is_where_its_span_says() {
    for entry in std::fs::read_dir("tests/files").unwrap() {
        let path = entry.unwrap().path();
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        if let Ok((cif, spans)) = read_with_spans(&text) {
            assert_spans_hold_the_document(&text, &cif, &spans);
        }
    }
}

#[test]
fn an_item_written_on_its_own_reads_back_as_itself() {
    let cif = read(MIXED).unwrap();

    for item in &cif.blocks[0].items {
        let written = item.written(Version::V1_1);
        assert!(written.ends_with('\n'), "{written:?}");
        let read_back = read(&format!("data_x\n{written}")).unwrap();
        assert_eq!(&read_back.blocks[0].items[0], item);
    }
}

#[test]
fn a_value_written_on_its_own_reads_back_as_itself() {
    let field = Value::Text {
        text: "two\nlines".into(),
        style: cif::Style::TextField,
    };

    assert_eq!(field.written(Version::V1_1, true), ";two\nlines\n;");
    assert_eq!(field.written(Version::V1_1, false), "\n;two\nlines\n;");
    for value in [
        Value::new("O'Brien"),
        Value::new("two words"),
        Value::unknown(),
        field,
    ] {
        let written = value.written(Version::V1_1, true);
        assert_eq!(read_value(&written, Version::V1_1).unwrap(), value);
    }
}

#[test]
fn a_shift_moves_what_follows_an_edit() {
    let text = "data_x\n_a 1\n_b 2\n";
    let (_, mut spans) = read_with_spans(text).unwrap();

    // `1` becomes `100`: two bytes more from the end of `1` on.
    spans.shift(11, 2);

    let edited = "data_x\n_a 100\n_b 2\n";
    assert_eq!(spans, read_with_spans(edited).unwrap().1);
}

#[test]
fn a_lone_value_reads_and_more_or_less_is_an_error() {
    assert_eq!(
        read_value(" 'a b' ", Version::V1_1).unwrap(),
        Value::new("a b")
    );
    assert!(read_value("", Version::V1_1).is_err());
    assert!(read_value("1 2", Version::V1_1).is_err());
}
