use cif::Parser;
use crystallib::Phase;

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    let bytes = std::fs::read(r"assets\BaTiO3.cif").unwrap();

    let data = Parser::new(&bytes).parse();

    dbg!(&data.keys().collect::<Vec<_>>());

    let phase: Phase = data.iter().next().unwrap().1.try_into().unwrap();

    dbg!(phase);
}
