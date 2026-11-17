#![doc = include_str!("../README.md")]

pub mod ddlm;
pub mod dictionary;
mod document;
pub mod number;
mod read;
mod spans;
mod write;

pub use document::{Cif, DataBlock, Item, Loop, Style, TableEntry, Value, Version};
pub use read::{read, read_value, read_with_spans, ReadError};
pub use spans::{BlockSpans, ItemSpans, Spans};
