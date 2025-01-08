use cif::Parser;

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Trace)
        .init();

    let bytes = std::fs::read(r"assets\BaTiO3.cif").unwrap();

    let data = Parser::new(&bytes).parse();

    for (name, value) in data.first_key_value().unwrap().1.iter() {
        println!("{}: {:?}", name, value);
    }
}
