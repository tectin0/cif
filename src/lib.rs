pub(crate) mod parse;
mod parser;
pub mod phase;

#[cfg(feature = "symmetry")]
pub mod symmetry;

pub use crystallib::Phase;
pub use parser::read_cif;
pub use parser::Cif;
pub use parser::Parser;

pub use parser::try_phase_from_cif_bytes;
