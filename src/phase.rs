use anyhow::Context;
use crystallib::{AdpType, Atom, Atoms, Cell, Phase};

use crate::{
    parse::GetAndParse,
    parser::DataBlock,
};

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

impl TryFrom<&DataBlock> for Phase {
    type Error = anyhow::Error;

    fn try_from(map: &DataBlock) -> anyhow::Result<Self> {
        Ok(Self {
            cell: Cell::try_from(map).context("Failed to parse cell")?,
            atoms: Atoms::try_from(map).context("Failed to parse atoms")?,
        })
    }
}

impl TryFrom<&DataBlock> for Cell {
    type Error = anyhow::Error;

    fn try_from(map: &DataBlock) -> anyhow::Result<Self> {
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

        let space_group = map
            .get_and_parse_first::<String>("_symmetry_space_group_name_H-M")
            .unwrap_or(map.get_and_parse_first::<String>("_space_group_name_H-M_alt")?);

        Ok(Self {
            a: values[0],
            b: values[1],
            c: values[2],
            alpha: values[3],
            beta: values[4],
            gamma: values[5],
            volume: values[6],
            space_group,
        })
    }
}

impl TryFrom<&DataBlock> for Atoms {
    type Error = anyhow::Error;

    fn try_from(map: &DataBlock) -> anyhow::Result<Self> {
        let label = map.get_and_parse_all::<String>("_atom_site_label")?;
        let type_ = map.get_and_parse_all::<String>("_atom_site_type_symbol")?;

        let x = map.get_and_parse_all::<f64>("_atom_site_fract_x")?;
        let y = map.get_and_parse_all::<f64>("_atom_site_fract_y")?;
        let z = map.get_and_parse_all::<f64>("_atom_site_fract_z")?;

        let occupancy = map.get_and_parse_all::<f64>("_atom_site_occupancy")?;
        let multiplicity = map
            .get_and_parse_all::<f64>("_atom_site_symmetry_multiplicity")
            .unwrap_or(map.get_and_parse_all::<f64>("_atom_site_site_symmetry_multiplicity")?);

        let u_iso_or_equiv = map
            .get_and_parse_all::<f64>("_atom_site_U_iso_or_equiv")
            .unwrap_or_default();

        let adp_type = map
            .get_and_parse_all::<AdpType>("_atom_site_adp_type")
            .unwrap_or(vec![AdpType::Uiso; label.len()]);

        let u11 = map
            .get_and_parse_all::<f64>("_atom_site_aniso_U_11")
            .unwrap_or_default();
        let u22 = map
            .get_and_parse_all::<f64>("_atom_site_aniso_U_22")
            .unwrap_or_default();
        let u33 = map
            .get_and_parse_all::<f64>("_atom_site_aniso_U_33")
            .unwrap_or_default();
        let u12 = map
            .get_and_parse_all::<f64>("_atom_site_aniso_U_12")
            .unwrap_or_default();
        let u13 = map
            .get_and_parse_all::<f64>("_atom_site_aniso_U_13")
            .unwrap_or_default();
        let u23 = map
            .get_and_parse_all::<f64>("_atom_site_aniso_U_23")
            .unwrap_or_default();

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
                u11: u11.get(index).cloned().unwrap_or_default(),
                u22: u22.get(index).cloned().unwrap_or_default(),
                u33: u33.get(index).cloned().unwrap_or_default(),
                u12: u12.get(index).cloned().unwrap_or_default(),
                u13: u13.get(index).cloned().unwrap_or_default(),
                u23: u23.get(index).cloned().unwrap_or_default(),
            };

            atoms.push(atom);
        }

        Ok(Self(atoms))
    }
}
