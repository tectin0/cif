#![doc = include_str!("../README.md")]

pub mod dictionary;
mod document;
pub mod number;
mod read;
mod write;

pub use document::{Cif, DataBlock, Item, Loop, Style, TableEntry, Value, Version};
pub use read::{read, ReadError};
