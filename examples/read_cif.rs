use cif::Parser;

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .init();

    let bytes = std::fs::read(r"assets\BaTiO3.cif").unwrap();

    let data = Parser::new(&bytes).parse();

    for (name, value) in data.iter() {
        println!("{}: {:?}", name, value);
    }
}
