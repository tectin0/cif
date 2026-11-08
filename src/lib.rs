//! CIF 1.1 documents, read and written without losing anything they hold.
//!
//! A [`Cif`] keeps its data blocks, items, loops and save frames in the order they were written,
//! each value with the way it was written ([`Style`]), and the comments between items. Writing a
//! read document reads back as the same document. Tags, block names and the reserved words are
//! compared ignoring case and kept as written.
//!
//! What a value means (a number with its standard uncertainty, a symmetry operator, ...) is left to
//! the reader of the document; [`number`] reads and writes CIF's numbers.

mod document;
pub mod number;
mod read;
mod write;

pub use document::{Cif, DataBlock, Item, Loop, Style, Value};
pub use read::{read, ReadError};
