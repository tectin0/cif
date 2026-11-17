//! DDLm dictionaries: a small one read on its own, and the embedded core dictionary.

use cif::ddlm::{CategoryClass, Dictionary};
use cif::{read, Version};

const TEMPLATES: &str = "#\\#CIF_2.0
data_TEMPL
save_length
    _description.text
;
     The length of an axis.
;
    _type.purpose     Measurand
    _type.contents    Real
    _units.code       angstroms
save_
";

const DICTIONARY: &str = "#\\#CIF_2.0
data_SMALL
    _dictionary.title  SMALL
    _dictionary.version 1.0
save_CELL
    _definition.id      CELL
    _definition.scope   Category
    _definition.class   Set
    _name.category_id   SMALL
save_
save_ATOM_SITE
    _definition.id      ATOM_SITE
    _definition.scope   Category
    _definition.class   Loop
    _category_key.name  '_atom_site.label'
    _name.category_id   SMALL
save_
save_cell.length_a
    _definition.id      '_cell.length_a'
    _alias.definition_id '_cell_length_a'
    _name.category_id   cell
    _name.object_id     length_a
    _import.get         [{'file':templ.cif 'save':length}]
save_
save_cell.length_a_su
    _definition.id      '_cell.length_a_su'
    _name.category_id   cell
    _name.object_id     length_a_su
    _type.purpose       SU
save_
save_atom_site.adp_type
    _definition.id      '_atom_site.ADP_type'
    loop_
      _alias.definition_id
      _alias.deprecation_date
         '_atom_site_ADP_type'                .
         '_atom_site_thermal_displace_type'   1999-03-24
    _name.category_id   atom_site
    _name.object_id     ADP_type
    _description.common 'Type of ADP'
    loop_
      _enumeration_set.state
      _enumeration_set.detail
         Uani  'anisotropic Uij'
         Uiso  'isotropic U'
save_
";

fn small() -> Dictionary {
    let templates = read(TEMPLATES).unwrap();
    let document = read(DICTIONARY).unwrap();
    Dictionary::read(&document, |file| {
        (file == "templ.cif").then_some(&templates)
    })
    .unwrap()
}

#[test]
fn a_definition_is_found_by_its_name_or_an_alias_and_takes_what_it_imports() {
    let dictionary = small();
    assert_eq!(dictionary.title, "SMALL");

    let a = dictionary.definition("_cell_length_a").unwrap();
    assert_eq!(dictionary.definition("_Cell.Length_A"), Some(a));
    assert_eq!(a.name, "_cell.length_a");
    assert_eq!(a.category, "cell");
    assert_eq!(a.contents.as_deref(), Some("Real"));
    assert_eq!(a.units.as_deref(), Some("angstroms"));
    assert_eq!(a.description.as_deref(), Some("The length of an axis."));
    assert!(!a.is_uncertainty());
    assert!(dictionary
        .definition("_cell.length_a_su")
        .unwrap()
        .is_uncertainty());
}

#[test]
fn a_tag_is_written_by_its_ddl1_alias_in_cif_1_1_and_its_ddlm_name_in_cif_2_0() {
    let dictionary = small();
    let adp_type = dictionary
        .definition("_atom_site_thermal_displace_type")
        .unwrap();

    assert_eq!(adp_type.name, "_atom_site.ADP_type");
    assert_eq!(adp_type.tag_for(Version::V1_1), "_atom_site_ADP_type");
    assert_eq!(adp_type.tag_for(Version::V2_0), "_atom_site.ADP_type");
    assert!(adp_type.aliases[1].deprecated);
    assert!(!adp_type.aliases[0].deprecated);

    let su = dictionary.definition("_cell.length_a_su").unwrap();
    assert_eq!(
        su.tag_for(Version::V1_1),
        "_cell.length_a_su",
        "without a DDL1 alias"
    );
}

#[test]
fn categories_are_sets_or_loops_with_their_keys_and_their_items() {
    let dictionary = small();

    let atom_site = dictionary.category("atom_site").unwrap();
    assert_eq!(atom_site.class, CategoryClass::Loop);
    assert_eq!(atom_site.keys, vec!["_atom_site.label".to_string()]);
    assert_eq!(
        dictionary.category("CELL").unwrap().class,
        CategoryClass::Set
    );

    let states: Vec<_> = dictionary
        .items_of("atom_site")
        .flat_map(|definition| definition.states.iter())
        .map(|state| (state.value.as_str(), state.detail.as_deref()))
        .collect();
    assert_eq!(
        states,
        vec![
            ("Uani", Some("anisotropic Uij")),
            ("Uiso", Some("isotropic U"))
        ]
    );
    assert_eq!(dictionary.items_of("cell").count(), 2);
}

#[test]
fn an_item_is_titled_by_its_common_name_else_by_its_ids() {
    let dictionary = small();

    let title = |tag| dictionary.definition(tag).unwrap().title();
    assert_eq!(title("_atom_site.ADP_type"), "Type of ADP");
    assert_eq!(title("_cell.length_a"), "Cell length a");
    assert_eq!(
        title("_cell.length_a_su"),
        "Cell length a standard uncertainty"
    );
}

#[test]
fn a_title_writes_abbreviations_out_and_keeps_the_case_of_symbols() {
    assert_eq!(
        cif::ddlm::title("atom_site_fract_x"),
        "Atom site fractional x"
    );
    assert_eq!(
        cif::ddlm::title("refine_ls_R_factor_all"),
        "Refine least-squares R factor all"
    );
    assert_eq!(
        cif::ddlm::title("atom_sites_Cartn_transform"),
        "Atom sites Cartesian transform"
    );
    assert_eq!(cif::ddlm::title("xray_symbol"), "X-ray symbol");
    assert_eq!(cif::ddlm::title(""), "");
}

#[test]
fn an_import_of_a_document_not_given_is_an_error() {
    let document = read(DICTIONARY).unwrap();
    let error = Dictionary::read(&document, |_| None).unwrap_err();

    assert!(error.message.contains("templ.cif"), "{}", error.message);
}

#[cfg(feature = "core-dictionary")]
mod core {
    use super::*;

    #[test]
    fn the_core_dictionary_reads_with_its_templates() {
        let started = std::time::Instant::now();
        let core = cif::ddlm::core();
        println!(
            "core dictionary: {} categories, {} definitions, read in {:?}",
            core.categories().len(),
            core.definitions().len(),
            started.elapsed()
        );

        let a = core.definition("_cell_length_a").unwrap();
        assert_eq!(core.definition("_cell.length_a"), Some(a));
        assert_eq!(a.contents.as_deref(), Some("Real"));
        assert_eq!(a.units.as_deref(), Some("angstroms"));
        assert!(a.description.is_some(), "imported from templ_attr.cif");
        assert_eq!(a.tag_for(Version::V1_1), "_cell_length_a");

        let atom_site = core.category("atom_site").unwrap();
        assert_eq!(atom_site.class, CategoryClass::Loop);
        assert_eq!(atom_site.keys, vec!["_atom_site.label".to_string()]);

        let adp_type = core.definition("_atom_site_thermal_displace_type").unwrap();
        assert_eq!(adp_type.name, "_atom_site.ADP_type");
        let states: Vec<&str> = adp_type.states.iter().map(|s| s.value.as_str()).collect();
        assert!(
            states.contains(&"Uani") && states.contains(&"Uiso"),
            "{states:?}"
        );
    }

    #[test]
    fn core_items_are_titled_by_their_ids() {
        let core = cif::ddlm::core();
        let title = |tag| core.definition(tag).unwrap().title();

        assert_eq!(title("_cell_angle_beta"), "Cell angle beta");
        assert_eq!(title("_cell_formula_units_Z"), "Cell formula units Z");
        assert_eq!(
            title("_atom_site_U_iso_or_equiv"),
            "Atom site U isotropic or equivalent"
        );
        assert_eq!(
            title("_diffrn_radiation_wavelength"),
            "Diffraction radiation wavelength value",
            "its DDLm name is `_diffrn_radiation_wavelength.value`"
        );
    }

    #[test]
    fn no_two_core_items_share_a_name_with_dots_read_as_underscores() {
        assert_eq!(cif::ddlm::core().ambiguous_names(), Vec::<&str>::new());
    }
}
