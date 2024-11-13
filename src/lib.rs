// TODO: find out where / how the site fraction is stored

pub(crate) mod parse;
pub mod phase;
mod read_cif;

pub use phase::Phase;
pub use read_cif::read_cif;
