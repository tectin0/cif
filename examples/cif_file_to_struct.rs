use cif::{phase::Phase, read_cif};

fn main() {
    let bytes = std::fs::read(r"assets\BaTiO3.cif").unwrap();

    let data = read_cif(bytes);

    let phase: Phase = (&data).try_into().unwrap();

    dbg!(phase);
}
