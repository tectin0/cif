use cif::Parser;
use crystallib::Phase;

#[test]
fn test_parse_all_local_files() {
    let entries = std::fs::read_dir("tests/files").unwrap();
    for entry in entries {
        let path = entry.unwrap().path();

        if path.extension().and_then(|e| e.to_str()) != Some("cif") {
            continue;
        }

        let bytes = std::fs::read(&path).unwrap();
        let cif = Parser::new(&bytes).parse();

        assert!(
            !cif.is_empty(),
            "Parsing {} produced no data blocks",
            path.display()
        );

        for (name, block) in cif.iter() {
            let phase: Phase = match block.try_into_phase() {
                Ok(p) => p,
                Err(e) => {
                    let causes: Vec<String> = e.chain().map(|e| e.to_string()).collect();
                    panic!(
                        "Failed to parse phase from {} block '{}': {}",
                        path.display(),
                        name,
                        causes.join(" -> ")
                    );
                }
            };
            assert!(
                !phase.atoms.is_empty(),
                "Phase from {} block '{}' has no atoms",
                path.display(),
                name
            );
        }
    }
}
