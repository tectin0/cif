use cif::Parser;
use crystallib::{AdpType, Atom, Atoms, Cell, Measured, Phase};

fn exact(value: f64) -> Measured {
    Measured::new(value)
}

fn with_esd(value: f64, esd: f64) -> Measured {
    Measured {
        value,
        esd: Some(esd),
    }
}

#[test]
fn test() {
    let bytes = std::fs::read(r"assets\BaTiO3.cif").unwrap();

    let data = Parser::new(&bytes).parse();

    let phase: Phase = data.first_key_value().unwrap().1.try_into().unwrap();

    let expected_phase = Phase {
        cell: Cell {
            a: with_esd(4.0094, 0.0002),
            b: with_esd(4.0094, 0.0002),
            c: with_esd(4.0094, 0.0002),
            alpha: exact(90.0),
            beta: exact(90.0),
            gamma: exact(90.0),
            volume: exact(64.45),
            space_group: "P m -3 m".to_string(),
            space_group_number: 221,
        },
        atoms: Atoms(vec![
            Atom {
                label: "Ba1".to_string(),
                type_: "Ba".to_string(),
                x: exact(0.0),
                y: exact(0.0),
                z: exact(0.0),
                occupancy: exact(1.0),
                multiplicity: Some(1.0),
                adp_type: AdpType::Uiso,
                u_iso_or_equiv: with_esd(0.0049, 0.0002),
                ..Default::default()
            },
            Atom {
                label: "Ti1".to_string(),
                type_: "Ti".to_string(),
                x: exact(0.5),
                y: exact(0.5),
                z: exact(0.5),
                occupancy: exact(1.0),
                multiplicity: Some(1.0),
                adp_type: AdpType::Uiso,
                u_iso_or_equiv: with_esd(0.0087, 0.0006),
                ..Default::default()
            },
            Atom {
                label: "O1".to_string(),
                type_: "O".to_string(),
                x: exact(0.5),
                y: exact(0.0),
                z: exact(0.5),
                occupancy: exact(1.0),
                multiplicity: Some(3.0),
                adp_type: AdpType::Uiso,
                u_iso_or_equiv: with_esd(0.005, 0.001),
                ..Default::default()
            },
        ]),
    };

    assert_eq!(phase, expected_phase);
}
