//! CIF 1.1's syntax, case by case.

use cif::{read, Item, Style, Value};

fn value(text: &str, style: Style) -> Value {
    Value {
        text: text.into(),
        style,
    }
}

#[test]
fn values_keep_the_way_they_were_written() {
    let cif = read(
        "data_x\n_a bare\n_b 'single quoted'\n_c \"double quoted\"\n_d ?\n_e .\n_f\n;line one\n  line two\n;\n",
    )
    .unwrap();
    let block = &cif.blocks[0];

    assert_eq!(block.value("_a"), Some(&value("bare", Style::Bare)));
    assert_eq!(block.value("_b"), Some(&value("single quoted", Style::SingleQuoted)));
    assert_eq!(block.value("_c"), Some(&value("double quoted", Style::DoubleQuoted)));
    assert_eq!(block.value("_d"), Some(&Value::unknown()));
    assert_eq!(block.value("_e"), Some(&Value::inapplicable()));
    assert_eq!(
        block.value("_f"),
        Some(&value("line one\n  line two", Style::TextField))
    );
}

#[test]
fn a_quote_ends_a_value_only_before_whitespace() {
    let cif = read("data_x\n_a 'O'Brien' _b 'it''s'\n").unwrap();
    let block = &cif.blocks[0];

    assert_eq!(block.value("_a").unwrap().text, "O'Brien");
    assert_eq!(block.value("_b").unwrap().text, "it''s");
}

#[test]
fn a_hash_starts_a_comment_only_outside_values() {
    let cif = read("data_x\n_a 'no # comment' # a comment\n_b\n;# not one either\n;\n_c a#b\n")
        .unwrap();
    let block = &cif.blocks[0];

    assert_eq!(block.value("_a").unwrap().text, "no # comment");
    assert_eq!(block.items[1], Item::Comment(" a comment".into()));
    assert_eq!(block.value("_b").unwrap().text, "# not one either");
    assert_eq!(block.value("_c").unwrap().text, "a#b");
}

#[test]
fn tags_and_reserved_words_ignore_case() {
    let cif = read("DATA_x\nLOOP_\n_Atom_Site_Label\nC1\n_CELL_length_a 1.0\n").unwrap();
    let block = &cif.blocks[0];

    assert_eq!(block.name, "x");
    assert!(block.loop_with("_atom_site_label").is_some());
    assert_eq!(block.value("_cell_length_a").unwrap().text, "1.0");
    assert!(cif.block("X").is_some());
}

#[test]
fn a_loop_keeps_its_tags_and_rows_in_order() {
    let cif = read("data_x\nloop_\n_b\n_a\n1 2\n3\n4\n").unwrap();
    let l = cif.blocks[0].loop_with("_a").unwrap();

    assert_eq!(l.tags, ["_b", "_a"]);
    assert_eq!(l.column_values("_a").unwrap(), [&Value::new("2"), &Value::new("4")]);
}

#[test]
fn values_may_follow_a_text_field_on_its_closing_line() {
    let cif = read("data_x\nloop_\n_a\n_b\n;text\n; after\n").unwrap();
    let l = cif.blocks[0].loop_with("_a").unwrap();

    assert_eq!(l.rows, [[value("text", Style::TextField), Value::new("after")]]);
}

#[test]
fn save_frames_and_comments_stay_where_they_were_written() {
    let cif = read("#\\#CIF_1.1\n# header\ndata_x\n_a 1\n# between\nsave_frame\n_b 2\nsave_\n").unwrap();

    assert_eq!(cif.comments, ["\\#CIF_1.1", " header"]);
    let items = &cif.blocks[0].items;
    assert_eq!(items[1], Item::Comment(" between".into()));
    assert!(matches!(&items[2], Item::SaveFrame { name, items } if name == "frame" && items.len() == 1));
}

#[test]
fn crlf_lines_and_a_byte_order_mark_read_as_plain_lines() {
    let cif = read("\u{feff}data_x\r\n_a 1\r\n_b\r\n;one\r\ntwo\r\n;\r\n").unwrap();

    assert_eq!(cif.blocks[0].value("_a").unwrap().text, "1");
    assert_eq!(cif.blocks[0].value("_b").unwrap().text, "one\ntwo");
}

#[test]
fn several_data_blocks_keep_their_order_and_their_names() {
    let cif = read("data_b\n_a 1\ndata_a\n_a 2\ndata_b\n_a 3\n").unwrap();

    let names: Vec<_> = cif.blocks.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, ["b", "a", "b"]);
}

#[test]
fn malformed_documents_say_what_is_wrong_and_where() {
    let error = |text: &str| read(text).unwrap_err();

    assert_eq!(error("data_x\n_a\n").line, 2);
    assert!(error("data_x\n_a\n").message.contains("without a value"));
    assert!(error("data_x\nloop_\n_a\n_b\n1 2 3\n").message.contains("whole number of rows"));
    assert!(error("data_x\nloop_\n_a\n_b\n").message.contains("without values"));
    assert!(error("data_x\n_a\n;open\n").message.contains("closing `;`"));
    assert!(error("data_x\n_a 'open\n").message.contains("closing '"));
    assert!(error("_a 1\n").message.contains("before the first data block"));
    assert!(error("data_x\n1\n").message.contains("without a tag"));
    assert!(error("data_x\nsave_f\n_a 1\n").message.contains("closing `save_`"));
    assert!(error("#\\#CIF_2.0\ndata_x\n").message.contains("CIF 2.0"));
    assert!(error("data_x\nglobal_\n").message.contains("reserved"));
}

#[test]
fn a_text_with_a_line_starting_with_a_semicolon_is_written_with_a_space_before_it() {
    let mut cif = read("data_x\n_a 1\n").unwrap();
    let Item::Pair { value, .. } = &mut cif.blocks[0].items[0] else {
        unreachable!()
    };
    *value = Value::new("one\n;two");

    let read_back = read(&cif.to_string()).unwrap();
    assert_eq!(read_back.blocks[0].value("_a").unwrap().text, "one\n ;two");
}
