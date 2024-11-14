// TODO: find out where / how the site fraction is stored

pub(crate) mod parse;
mod parser;
pub mod phase;

pub use parser::read_cif;
pub use parser::Parser;
pub use phase::Phase;
