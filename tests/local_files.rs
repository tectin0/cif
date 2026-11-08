//! Every CIF in `assets/` and the untracked `tests/files/` reads back as itself once written. A
//! check on whatever files are at hand, next to `round_trip.rs`'s generated documents.

use std::path::Path;

#[test]
fn every_local_cif_reads_back_as_itself() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut checked = 0;

    for dir in [root.join("assets"), root.join("tests").join("files")] {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for path in entries.map(|entry| entry.unwrap().path()) {
            if path.extension().is_none_or(|ext| !ext.eq_ignore_ascii_case("cif")) {
                continue;
            }
            let text = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
            let cif = cif::read(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert_eq!(
                cif::read(&cif.to_string()).as_ref(),
                Ok(&cif),
                "{}",
                path.display()
            );
            checked += 1;
        }
    }

    assert!(checked > 0, "no CIF found in assets/");
}
