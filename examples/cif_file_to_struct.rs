use cif::{phase::Phase, Parser};

fn main() {
    let bytes = std::fs::read(r"assets\BaTiO3.cif").unwrap();

    let data = Parser::new(&bytes).parse();

    let phase: Phase = (&data).try_into().unwrap();

    dbg!(phase);
}
