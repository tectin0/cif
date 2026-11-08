# cif

Reads and writes [CIF 1.1](https://www.iucr.org/resources/cif/spec/version1.1) (Crystallographic
Information File) documents without losing anything they hold.

A `Cif` keeps its data blocks, items, loops and save frames in the order they were written, each
value with the way it was written (bare, quoted, in a text field, `?` or `.`), and the comments
between items. Writing a document that was read gives a document that reads back as the same one,
so a program can change the values it knows and pass everything else through untouched. Tags,
block names and the reserved words are compared ignoring case and kept as written.

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
```

The optional `serde` feature derives `Serialize` and `Deserialize` for the document types.

CIF 2.0 documents are rejected with an error.

## License

GPL-3.0-only
