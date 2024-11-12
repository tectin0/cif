// TODO: space group still has a `'` at the beginning -> need to remove it

pub mod phase;
mod read_cif;

pub use phase::Phase;
pub use read_cif::read_cif;
