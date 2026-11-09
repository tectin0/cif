//! CIF 1.1's and 2.0's syntax, case by case.

use cif::{read, Item, Style, TableEntry, Value, Version};

fn value(text: &str, style: Style) -> Value {
    Value::Text {
        text: text.into(),
        style,
    }
}

/// The text of the value of `tag` in the first block of `cif`.
fn text<'a>(cif: &'a cif::Cif, tag: &str) -> &'a str {
    cif.blocks[0].value(tag).and_then(Value::text).unwrap()
}

#[test]
fn values_keep_the_way_they_were_written() {
    let cif = read(
        "data_x\n_a bare\n_b 'single quoted'\n_c \"double quoted\"\n_d ?\n_e .\n_f\n;line one\n  line two\n;\n",
    )
    .unwrap();
    let block = &cif.blocks[0];

    assert_eq!(cif.version, Version::V1_1);
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
fn in_cif_1_1_a_quote_ends_a_value_only_before_whitespace() {
    let cif = read("data_x\n_a 'O'Brien' _b 'it''s'\n").unwrap();

    assert_eq!(text(&cif, "_a"), "O'Brien");
    assert_eq!(text(&cif, "_b"), "it''s");
}

#[test]
fn a_hash_starts_a_comment_only_outside_values() {
    let cif = read("data_x\n_a 'no # comment' # a comment\n_b\n;# not one either\n;\n_c a#b\n")
        .unwrap();

    assert_eq!(text(&cif, "_a"), "no # comment");
    assert_eq!(cif.blocks[0].items[1], Item::Comment(" a comment".into()));
    assert_eq!(text(&cif, "_b"), "# not one either");
    assert_eq!(text(&cif, "_c"), "a#b");
}

#[test]
fn tags_and_reserved_words_ignore_case() {
    let cif = read("DATA_x\nLOOP_\n_Atom_Site_Label\nC1\n_CELL_length_a 1.0\n").unwrap();
    let block = &cif.blocks[0];

    assert_eq!(block.name, "x");
    assert!(block.loop_with("_atom_site_label").is_some());
    assert_eq!(text(&cif, "_cell_length_a"), "1.0");
    assert!(cif.block("X").is_some());
}

#[test]
fn a_ddlm_tag_is_the_same_as_its_ddl1_form() {
    let cif = read("data_x\n_cell.length_a 1.0\nloop_\n_atom_site.label\nC1\n").unwrap();
    let block = &cif.blocks[0];

    assert_eq!(block.pair("_cell_length_a"), Some(&Value::new("1.0")));
    assert!(block.loop_with("_atom_site_label").is_some());
}

#[test]
fn a_pair_is_found_by_its_tag_and_a_single_row_loop_is_not_a_pair() {
    let cif = read("data_x\n_a 1\nloop_\n_b\n2\n").unwrap();
    let block = &cif.blocks[0];

    assert_eq!(block.pair("_a"), Some(&Value::new("1")));
    assert_eq!(block.pair("_b"), None);
    assert_eq!(block.value("_b"), Some(&Value::new("2")));
}

#[test]
fn a_new_number_keeps_the_value_as_written_while_it_reads_as_that_number() {
    let written = value("5.4321(12)", Style::SingleQuoted);

    assert_eq!(written.number(), Some((5.4321, Some(0.0012))));
    assert_eq!(written.with_number(5.4321, Some(0.0012)), written);
    assert_eq!(
        written.with_number(5.5, None),
        value("5.5", Style::SingleQuoted),
        "a changed number is written in the value's style"
    );
    assert_eq!(Value::unknown().number(), None);
}

#[test]
fn an_emptied_value_is_written_as_unknown_unless_it_is_unknown_or_inapplicable() {
    assert_eq!(Value::new("1").emptied(), Value::unknown());
    assert_eq!(Value::inapplicable().emptied(), Value::inapplicable());
}

#[test]
fn a_block_name_holds_no_whitespace() {
    assert_eq!(cif::DataBlock::name_for("Ba Ti O3"), "Ba_Ti_O3");
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

    assert_eq!(text(&cif, "_a"), "1");
    assert_eq!(text(&cif, "_b"), "one\ntwo");
}

#[test]
fn several_data_blocks_keep_their_order_and_their_names() {
    let cif = read("data_b\n_a 1\ndata_a\n_a 2\ndata_b\n_a 3\n").unwrap();

    let names: Vec<_> = cif.blocks.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, ["b", "a", "b"]);
}

#[test]
fn a_cif_2_0_document_is_read_with_its_lists_tables_and_triple_quoted_text() {
    let cif = read(
        "#\\#CIF_2.0\ndata_x\n_list [1 'two' [3 ?]]\n_table {'a':1 \"b\": [x y]}\n_long '''one\ntwo'''\n_empty []\n",
    )
    .unwrap();
    let block = &cif.blocks[0];

    assert_eq!(cif.version, Version::V2_0);
    assert!(cif.comments.is_empty(), "the magic line is the version");
    assert_eq!(
        block.value("_list"),
        Some(&Value::List(vec![
            Value::new("1"),
            value("two", Style::SingleQuoted),
            Value::List(vec![Value::new("3"), Value::unknown()]),
        ]))
    );
    assert_eq!(
        block.value("_table"),
        Some(&Value::Table(vec![
            TableEntry {
                key: "a".into(),
                key_style: Style::SingleQuoted,
                value: Value::new("1"),
            },
            TableEntry {
                key: "b".into(),
                key_style: Style::DoubleQuoted,
                value: Value::List(vec![Value::new("x"), Value::new("y")]),
            },
        ]))
    );
    assert_eq!(block.value("_long"), Some(&value("one\ntwo", Style::TripleSingleQuoted)));
    assert_eq!(block.value("_empty"), Some(&Value::List(Vec::new())));
}

#[test]
fn in_cif_2_0_a_quote_always_ends_a_value_and_brackets_end_a_bare_one() {
    let cif = read("#\\#CIF_2.0\ndata_x\n_a 'x'y\n_b [a]\n").unwrap_err();
    assert!(cif.message.contains("without a tag"), "{cif}");

    let cif = read("#\\#CIF_2.0\ndata_x\nloop_\n_a\n_b\n_c\n'x' a[1]\n").unwrap();
    let l = cif.blocks[0].loop_with("_a").unwrap();
    assert_eq!(
        l.rows,
        [[
            value("x", Style::SingleQuoted),
            Value::new("a"),
            Value::List(vec![Value::new("1")]),
        ]]
    );
}

#[test]
fn a_cif_1_1_document_reads_brackets_as_text() {
    let cif = read("data_x\n_a [1 _b x\n").unwrap();

    assert_eq!(text(&cif, "_a"), "[1");
}

#[test]
fn a_1_1_document_with_a_list_is_written_as_cif_2_0() {
    let mut cif = read("data_x\n_a 1\n").unwrap();
    let Item::Pair { value, .. } = &mut cif.blocks[0].items[0] else {
        unreachable!()
    };
    *value = Value::List(vec![Value::new("1"), Value::new("2")]);

    let written = cif.to_string();
    assert!(written.starts_with("#\\#CIF_2.0\n"), "{written}");
    assert_eq!(read(&written).unwrap().version, Version::V2_0);
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
    assert!(error("data_x\nglobal_\n").message.contains("reserved"));

    let v2 = |body: &str| error(&format!("#\\#CIF_2.0\ndata_x\n{body}\n"));
    assert!(v2("_a [1 2").message.contains("closing `]`"));
    assert!(v2("_a {'k':1").message.contains("closing `}`"));
    assert!(v2("_a {k:1}").message.contains("must be quoted"));
    assert!(v2("_a {'k' 1}").message.contains("without its `:`"));
    assert!(v2("_a '''open").message.contains("closing '''"));
    assert!(v2("_a ]").message.contains("closes no list"));
}

#[test]
fn a_text_with_a_line_starting_with_a_semicolon_is_written_with_a_space_before_it() {
    let mut cif = read("data_x\n_a 1\n").unwrap();
    let Item::Pair { value, .. } = &mut cif.blocks[0].items[0] else {
        unreachable!()
    };
    *value = Value::new("one\n;two");

    let read_back = read(&cif.to_string()).unwrap();
    assert_eq!(text(&read_back, "_a"), "one\n ;two");
}
