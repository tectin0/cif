use cif::read_cif;

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .init();

    let bytes = std::fs::read(r"assets\BaTiO3.cif").unwrap();

    let data = read_cif(bytes);

    for (name, value) in data.iter() {
        println!("{}: {:?}", name, value);
    }
}
