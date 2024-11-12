use cif::read_cif;

fn main() {
    let bytes = std::fs::read(r"assets\BaTiO3.cif").unwrap();

    let data = read_cif(bytes);

    for (name, value) in data.iter() {
        println!("{}: {:?}", name, value);
    }
}
