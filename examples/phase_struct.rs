use cif::phase::{Cell, Phase};

fn main() {
    let cell = Cell::default();

    let cell_as_string = serde_json::to_string_pretty(&cell).unwrap();

    println!("{}", cell_as_string);
}
