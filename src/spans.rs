//! Where the parts of a document are in the text it was read from (see
//! [`read_with_spans`](crate::read_with_spans)): byte ranges, in a tree that mirrors the document's.

use std::ops::Range;

/// Where a document's parts are in its text, block by block.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Spans {
    /// The comments before the first data block, one per [`Cif::comments`](crate::Cif::comments).
    pub comments: Vec<Range<usize>>,
    /// One per [`Cif::blocks`](crate::Cif::blocks).
    pub blocks: Vec<BlockSpans>,
}

/// Where a data block's parts are.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlockSpans {
    /// One per [`DataBlock::comments`](crate::DataBlock::comments).
    pub comments: Vec<Range<usize>>,
    /// `data_<name>`.
    pub header: Range<usize>,
    /// One per [`DataBlock::items`](crate::DataBlock::items).
    pub items: Vec<ItemSpans>,
}

/// Where an item's parts are, as [`Item`](crate::Item) has them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemSpans {
    Pair {
        tag: Range<usize>,
        value: Range<usize>,
    },
    Loop {
        /// `loop_`.
        keyword: Range<usize>,
        tags: Vec<Range<usize>>,
        /// The values row by row, as [`Loop::rows`](crate::Loop::rows) has them one after the other.
        values: Vec<Range<usize>>,
    },
    SaveFrame {
        /// `save_<name>`.
        header: Range<usize>,
        items: Vec<ItemSpans>,
        /// The closing `save_`.
        end: Range<usize>,
    },
    /// From the `#` to the end of its line.
    Comment(Range<usize>),
}

impl ItemSpans {
    /// From the item's first character to its last. A comment written among a pair's or a loop's
    /// values lies within it, though the document keeps it as an item of its own after them.
    pub fn whole(&self) -> Range<usize> {
        match self {
            ItemSpans::Pair { tag, value } => tag.start..value.end,
            ItemSpans::Loop {
                keyword,
                tags,
                values,
            } => {
                let end = values.last().or(tags.last()).unwrap_or(keyword).end;
                keyword.start..end
            }
            ItemSpans::SaveFrame { header, end, .. } => header.start..end.end,
            ItemSpans::Comment(range) => range.clone(),
        }
    }

    fn ranges_mut(&mut self, f: &mut impl FnMut(&mut Range<usize>)) {
        match self {
            ItemSpans::Pair { tag, value } => {
                f(tag);
                f(value);
            }
            ItemSpans::Loop {
                keyword,
                tags,
                values,
            } => {
                f(keyword);
                tags.iter_mut().for_each(&mut *f);
                values.iter_mut().for_each(&mut *f);
            }
            ItemSpans::SaveFrame { header, items, end } => {
                f(header);
                for item in items {
                    item.ranges_mut(f);
                }
                f(end);
            }
            ItemSpans::Comment(range) => f(range),
        }
    }

    /// Moves every bound at or after `at` by `delta` bytes (see [`Spans::shift`]).
    pub fn shift(&mut self, at: usize, delta: isize) {
        self.ranges_mut(&mut |range| shift_range(range, at, delta));
    }
}

fn shift_range(range: &mut Range<usize>, at: usize, delta: isize) {
    let shift = |bound: &mut usize| {
        if *bound >= at {
            *bound = bound.saturating_add_signed(delta);
        }
    };
    shift(&mut range.start);
    shift(&mut range.end);
}

impl Spans {
    /// Moves every bound at or after `at` by `delta` bytes, as text inserted (`delta > 0`) or
    /// removed (`delta < 0`) at `at` moves what follows it. A range ending at `at` moves its end
    /// too, so `at` shouldn't be where a range ends that stays where it is.
    pub fn shift(&mut self, at: usize, delta: isize) {
        for range in &mut self.comments {
            shift_range(range, at, delta);
        }
        for block in &mut self.blocks {
            for range in &mut block.comments {
                shift_range(range, at, delta);
            }
            shift_range(&mut block.header, at, delta);
            for item in &mut block.items {
                item.shift(at, delta);
            }
        }
    }
}
