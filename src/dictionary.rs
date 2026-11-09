//! What the core, msCIF and magCIF dictionaries say about some of their items: the names that
//! replaced older ones, which items name an atom site, and conventions for reading atom sites.

use std::f64::consts::PI;

/// The factor between the B and U forms of a displacement parameter: B = 8π²U.
pub const B_PER_U: f64 = 8.0 * PI * PI;

/// Older tags and the current tags that replaced them, as [`canonical`] writes both.
const REPLACED_TAGS: &[(&str, &str)] = &[
    (
        "_symmetry_space_group_name_h-m",
        "_space_group_name_h-m_alt",
    ),
    ("_symmetry_space_group_name_hall", "_space_group_name_hall"),
    ("_symmetry_int_tables_number", "_space_group_it_number"),
    (
        "_symmetry_equiv_pos_as_xyz",
        "_space_group_symop_operation_xyz",
    ),
    (
        "_atom_site_symmetry_multiplicity",
        "_atom_site_site_symmetry_multiplicity",
    ),
    ("_atom_site_thermal_displace_type", "_atom_site_adp_type"),
];

/// The tags whose values name an atom site by its `_atom_site_label`, from the links the core,
/// msCIF and magCIF dictionaries declare, as [`canonical`] writes them. `_atom_site_aniso_label`
/// is among them.
const ATOM_SITE_LABEL_REFERENCES: &[&str] = &[
    "_atom_site_aniso_label",
    "_atom_site_calc_attached_atom",
    "_geom_angle_atom_site_label_1",
    "_geom_angle_atom_site_label_2",
    "_geom_angle_atom_site_label_3",
    "_geom_bond_atom_site_label_1",
    "_geom_bond_atom_site_label_2",
    "_geom_contact_atom_site_label_1",
    "_geom_contact_atom_site_label_2",
    "_geom_hbond_atom_site_label_d",
    "_geom_hbond_atom_site_label_h",
    "_geom_hbond_atom_site_label_a",
    "_geom_torsion_atom_site_label_1",
    "_geom_torsion_atom_site_label_2",
    "_geom_torsion_atom_site_label_3",
    "_geom_torsion_atom_site_label_4",
    "_atom_site_displace_fourier_atom_site_label",
    "_atom_site_displace_special_func_atom_site_label",
    "_atom_site_occ_fourier_atom_site_label",
    "_atom_site_occ_special_func_atom_site_label",
    "_atom_site_u_fourier_atom_site_label",
    "_atom_site_rot_fourier_atom_site_label",
    "_atom_site_moment_label",
    "_atom_site_moment_fourier_atom_site_label",
    "_atom_site_moment_special_func_atom_site_label",
];

/// `tag` by its current name, lowercase and with DDLm's `.` read as `_`: an older tag is written
/// as the one that replaced it (`_symmetry_int_tables_number` is `_space_group_it_number`).
pub fn canonical(tag: &str) -> String {
    let tag = tag.to_ascii_lowercase().replace('.', "_");
    match REPLACED_TAGS.iter().find(|(older, _)| *older == tag) {
        Some((_, current)) => current.to_string(),
        None => tag,
    }
}

/// Whether the values of `tag` name an atom site by its `_atom_site_label`.
pub fn names_atom_site(tag: &str) -> bool {
    ATOM_SITE_LABEL_REFERENCES.contains(&canonical(tag).as_str())
}

/// The element an atom site's label starts with (`Ba1` is `Ba`, `O2` is `O`), the type symbol of
/// a site without `_atom_site_type_symbol`.
pub fn type_symbol_from_label(label: &str) -> String {
    let letters: String = label
        .chars()
        .take_while(char::is_ascii_alphabetic)
        .take(2)
        .collect();
    let mut chars = letters.chars();
    match chars.next() {
        Some(first) => {
            first.to_ascii_uppercase().to_string() + &chars.as_str().to_ascii_lowercase()
        }
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_older_or_ddlm_tag_is_read_by_its_current_name() {
        assert_eq!(
            canonical("_symmetry_Int_Tables_number"),
            "_space_group_it_number"
        );
        assert_eq!(canonical("_atom_site.fract_x"), "_atom_site_fract_x");
        assert_eq!(canonical("_cell_length_a"), "_cell_length_a");
    }

    #[test]
    fn a_bond_names_atom_sites_and_a_coordinate_does_not() {
        assert!(names_atom_site("_geom_bond.atom_site_label_1"));
        assert!(!names_atom_site("_atom_site_fract_x"));
    }

    #[test]
    fn a_label_s_letters_are_its_type_symbol() {
        assert_eq!(type_symbol_from_label("BA1"), "Ba");
        assert_eq!(type_symbol_from_label("O2a"), "O");
        assert_eq!(type_symbol_from_label("12"), "");
    }
}
