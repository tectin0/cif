# cif

Reads and writes [CIF 1.1](https://www.iucr.org/resources/cif/spec/version1.1) and
[CIF 2.0](https://doi.org/10.1107/S1600576715021871) (Crystallographic Information File) documents
without losing anything they hold.

A `Cif` keeps its version, its data blocks, items, loops and save frames in the order they were
written, each value with the way it was written (bare, quoted, triple-quoted, in a text field, `?`
or `.`), CIF 2.0's lists and tables, and the comments between items. Writing a document that was
read gives a document that reads back as the same one, in the same version, so a program can
change the values it knows and pass everything else through untouched. Tags, block names and the
reserved words are compared ignoring case and kept as written.

A document is CIF 2.0 if its first line is `#\#CIF_2.0`, and CIF 1.1 otherwise; each is read and
written with its own rules (in CIF 1.1 a quote ends a value only before whitespace, in CIF 2.0 it
always does). A CIF 1.1 document given a list or table is written as CIF 2.0, the only version that
can hold one.

What a value means is left to the program reading the document; `cif::number` reads and writes
CIF's numbers with their standard uncertainty (`5.4321(12)`).

```rust
let text = "data_NaCl
_cell_length_a 5.6402(3)
loop_
_atom_site_label
_atom_site_fract_x
Na1 0
Cl1 0.5
";
let mut cif = cif::read(text).expect("a CIF 1.1 document");

let block = &cif.blocks[0];
let a = block.value("_cell_length_a").and_then(|v| v.as_str()).unwrap();
assert_eq!(cif::number::parse(a), Some((5.6402, Some(0.0003))));
let labels = block.loop_with("_atom_site_label").unwrap();
assert_eq!(labels.rows.len(), 2);

// Change one value; everything else is written as it was read.
if let Some(cif::Item::Pair { value, .. }) = cif.blocks[0].items.first_mut() {
    *value = value.with_text(cif::number::format(5.6431, Some(0.0002)));
}
let written = cif.to_string();
assert!(written.contains("_cell_length_a 5.6431(2)"));
assert_eq!(cif::read(&written).unwrap(), cif);

// CIF 2.0: lists, tables and triple-quoted text.
let cif = cif::read("#\\#CIF_2.0\ndata_x\n_hkl [1 0 0]\n_note '''two\nlines'''\n").unwrap();
assert_eq!(cif.version, cif::Version::V2_0);
let cif::Value::List(hkl) = cif.blocks[0].value("_hkl").unwrap() else {
    unreachable!()
};
assert_eq!(hkl.len(), 3);
```

The optional `serde` feature derives `Serialize` and `Deserialize` for the document types.

## License

GPL-3.0-only
