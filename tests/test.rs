use cif::{
    phase::{Atom, Atoms, Cell, Uaniso}, Parser, Phase,
};

#[test]
fn test() {
    let bytes = std::fs::read(r"assets\BaTiO3.cif").unwrap();

    let data = Parser::new(&bytes).parse();

    let phase: Phase = (&data).try_into().unwrap();

    let expected_phase = Phase {
        cell: Cell {
            a: 4.0094,
            b: 4.0094,
            c: 4.0094,
            alpha: 90.0,
            beta: 90.0,
            gamma: 90.0,
            volume: 64.45,
            space_group: "P m -3 m".to_string(),
        },
        atoms: Atoms(vec![
            Atom {
                label: "Ba1".to_string(),
                type_: "Ba".to_string(),
                x: 0.0,
                y: 0.0,
                z: 0.0,
                occupancy: 1.0,
                multiplicity: 1.0,
                adp_type: "Uiso".to_string(),
                u_iso_or_equiv: 0.0049,
                u_aniso: Uaniso {
                    u11: 0.0,
                    u22: 0.0,
                    u33: 0.0,
                    u12: 0.0,
                    u13: 0.0,
                    u23: 0.0,
                },
                site_fraction: 1.0,
            },
            Atom {
                label: "Ti1".to_string(),
                type_: "Ti".to_string(),
                x: 0.5,
                y: 0.5,
                z: 0.5,
                occupancy: 1.0,
                multiplicity: 1.0,
                adp_type: "Uiso".to_string(),
                u_iso_or_equiv: 0.0087,
                u_aniso: Uaniso {
                    u11: 0.0,
                    u22: 0.0,
                    u33: 0.0,
                    u12: 0.0,
                    u13: 0.0,
                    u23: 0.0,
                },
                site_fraction: 1.0,
            },
            Atom {
                label: "O1".to_string(),
                type_: "O".to_string(),
                x: 0.5,
                y: 0.0,
                z: 0.5,
                occupancy: 1.0,
                multiplicity: 3.0,
                adp_type: "Uiso".to_string(),
                u_iso_or_equiv: 0.005,
                u_aniso: Uaniso {
                    u11: 0.0,
                    u22: 0.0,
                    u33: 0.0,
                    u12: 0.0,
                    u13: 0.0,
                    u23: 0.0,
                },
                site_fraction: 1.0,
            },
        ]),
    };

    assert_eq!(phase, expected_phase);
}
