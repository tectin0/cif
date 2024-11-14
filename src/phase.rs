use crate::{parse::GetAndParse, parser::Cif};

use anyhow::Context;

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Default, Debug, Clone, PartialEq)]
pub struct Phase {
    pub cell: Cell,
    pub atoms: Atoms,
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Default, Debug, Clone, PartialEq)]
pub struct Cell {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub alpha: f64,
    pub beta: f64,
    pub gamma: f64,
    pub volume: f64,
    pub space_group: String,
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Atoms(pub Vec<Atom>);

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Atom {
    pub label: String,
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    pub type_: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub occupancy: f64,
    pub multiplicity: f64,
    pub adp_type: String,
    pub u_iso_or_equiv: f64,
    pub u_aniso: Uaniso,
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Uaniso {
    #[cfg_attr(feature = "serde", serde(rename = "U11"))]
    pub u11: f64,
    #[cfg_attr(feature = "serde", serde(rename = "U22"))]
    pub u22: f64,
    #[cfg_attr(feature = "serde", serde(rename = "U33"))]
    pub u33: f64,
    #[cfg_attr(feature = "serde", serde(rename = "U12"))]
    pub u12: f64,
    #[cfg_attr(feature = "serde", serde(rename = "U13"))]
    pub u13: f64,
    #[cfg_attr(feature = "serde", serde(rename = "U23"))]
    pub u23: f64,
}

impl TryFrom<&Cif> for Phase {
    type Error = anyhow::Error;

    fn try_from(map: &Cif) -> anyhow::Result<Self> {
        Ok(Self {
            cell: Cell::try_from(map).context("Failed to parse cell")?,
            atoms: Atoms::try_from(map).context("Failed to parse atoms")?,
        })
    }
}

impl TryFrom<&Cif> for Cell {
    type Error = anyhow::Error;

    fn try_from(map: &Cif) -> anyhow::Result<Self> {
        let values = [
            "_cell_length_a",
            "_cell_length_b",
            "_cell_length_c",
            "_cell_angle_alpha",
            "_cell_angle_beta",
            "_cell_angle_gamma",
            "_cell_volume",
        ]
        .map(|key| map.get_and_parse_first::<f64>(key))
        .into_iter()
        .collect::<Result<Vec<f64>, _>>()?;

        Ok(Self {
            a: values[0],
            b: values[1],
            c: values[2],
            alpha: values[3],
            beta: values[4],
            gamma: values[5],
            volume: values[6],
            space_group: map.get_and_parse_first::<String>("_symmetry_space_group_name_H-M")?,
        })
    }
}

impl TryFrom<&Cif> for Atoms {
    type Error = anyhow::Error;

    fn try_from(map: &Cif) -> anyhow::Result<Self> {
        let label = map.get_and_parse_all::<String>("_atom_site_label")?;
        let type_ = map.get_and_parse_all::<String>("_atom_site_type_symbol")?;

        let x = map.get_and_parse_all::<f64>("_atom_site_fract_x")?;
        let y = map.get_and_parse_all::<f64>("_atom_site_fract_y")?;
        let z = map.get_and_parse_all::<f64>("_atom_site_fract_z")?;

        let occupancy = map.get_and_parse_all::<f64>("_atom_site_occupancy")?;
        let multiplicity = map.get_and_parse_all::<f64>("_atom_site_symmetry_multiplicity")?;

        let u_iso_or_equiv = map
            .get_and_parse_all::<f64>("_atom_site_U_iso_or_equiv")
            .unwrap_or_default();

        let adp_type = map
            .get_and_parse_all::<String>("_atom_site_adp_type")
            .unwrap_or(vec!["Uiso".to_string(); label.len()]);

        let u_aniso_values = [
            "_atom_site_aniso_U_11",
            "_atom_site_aniso_U_22",
            "_atom_site_aniso_U_33",
            "_atom_site_aniso_U_12",
            "_atom_site_aniso_U_13",
            "_atom_site_aniso_U_23",
        ]
        .map(|key| map.get_and_parse_all::<f64>(key))
        .into_iter()
        .collect::<Result<Vec<Vec<f64>>, _>>()
        .unwrap_or_default();

        let mut u_aniso = Vec::new();

        if !u_aniso_values.is_empty() {
            for u in 0..u_aniso_values[0].len() {
                u_aniso.push(Uaniso {
                    u11: u_aniso_values[0][u],
                    u22: u_aniso_values[1][u],
                    u33: u_aniso_values[2][u],
                    u12: u_aniso_values[3][u],
                    u13: u_aniso_values[4][u],
                    u23: u_aniso_values[5][u],
                });
            }
        }

        let mut atoms = Vec::new();

        for (index, label) in label.into_iter().enumerate() {
            let atom = Atom {
                label,
                type_: type_[index].clone(),
                x: x[index],
                y: y[index],
                z: z[index],
                occupancy: occupancy[index],
                multiplicity: multiplicity[index],
                adp_type: adp_type[index].clone(),
                u_iso_or_equiv: u_iso_or_equiv.get(index).cloned().unwrap_or_default(),
                u_aniso: u_aniso.get(index).cloned().unwrap_or_default(),
            };

            atoms.push(atom);
        }

        Ok(Self(atoms))
    }
}
