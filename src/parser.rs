#![deny(elided_lifetimes_in_paths)]
use std::{collections::BTreeMap, io::IsTerminal};

use anyhow::Context;

#[derive(Debug)]
struct GlobalFlags {
    is_loop: bool,
    is_string_multiline: bool,
    is_string_with_spaces: bool,
    is_new_line: bool,
    is_comment: bool,
}

impl Default for GlobalFlags {
    fn default() -> Self {
        Self {
            is_loop: false,
            is_string_multiline: false,
            is_string_with_spaces: false,
            is_new_line: true,
            is_comment: false,
        }
    }
}

#[derive(Default, Debug)]
struct LocalFlags {
    has_new_line_byte: bool,
    is_loop_chunk: bool,
    is_data_chunk: bool,
}

#[derive(Default, Debug)]
struct TempData {
    names: Vec<String>,
    values: Vec<String>,
}

pub struct Parser<'a> {
    chunks: Box<dyn Iterator<Item = &'a [u8]> + 'a>,
    chunk: Option<&'a [u8]>,
    data: BTreeMap<String, Vec<String>>,
    string_multiline: Vec<u8>,
    string_with_spaces: Vec<u8>,
    temp_data: TempData,
    values_cleared_this_loop: usize,
    global_flags: GlobalFlags,
    local_flags: LocalFlags,
}

impl std::fmt::Debug for Parser<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Parser")
            .field("chunk", &self.chunk)
            .field("data", &self.data)
            .field("string_multiline", &self.string_multiline)
            .field("string_with_spaces", &self.string_with_spaces)
            .field("temp_data", &self.temp_data)
            .field("values_cleared_this_loop", &self.values_cleared_this_loop)
            .field("global_flags", &self.global_flags)
            .field("local_flags", &self.local_flags)
            .finish()
    }
}

impl<'a> Parser<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        let chunks = Box::new(split_input_into_chunks(bytes));

        Self {
            chunks,
            chunk: None,
            data: BTreeMap::new(),
            string_multiline: Vec::new(),
            string_with_spaces: Vec::new(),
            temp_data: TempData::default(),
            values_cleared_this_loop: 0,
            global_flags: GlobalFlags::default(),
            local_flags: LocalFlags::default(),
        }
    }

    pub fn parse(&mut self) -> Cif {
        log::debug!("Parsing CIF file");

        self.skip_until_first_data_chunk();

        while self.chunk.is_some() {
            self.update_data_from_temp_data()
                .and_then(|()| self.check_is_empty())
                .and_then(|()| self.check_has_new_line_byte())
                .and_then(|()| self.check_comment_byte())
                .and_then(|()| self.check_comment_ends())
                .and_then(|()| self.check_is_comment())
                .and_then(|()| self.chunk_trim_ascii_end())
                .and_then(|()| self.check_is_empty())
                .and_then(|()| self.check_is_start_or_end_of_multi_line_string())
                .and_then(|()| self.handle_if_multi_line_string())
                .and_then(|()| self.handle_multi_line_string_end())
                .and_then(|()| self.check_is_string_with_spaces())
                .and_then(|()| self.handle_if_string_with_spaces())
                .and_then(|()| self.check_loop_byte())
                .and_then(|()| self.check_is_data_chunk())
                .and_then(|()| self.check_loop_end())
                .and_then(|()| self.handle_if_data_chunk())
                .and_then(|()| self.handle_if_value_chunk())
                .unwrap_or(());

            log::debug!("------{:?}", self.global_flags);
            log::debug!("------{:?}", self.local_flags);
            log::debug!("------No. Names {:?}", self.temp_data.names.len());
            log::debug!("------No. Values {:?}", self.temp_data.values.len());

            self.next();
        }

        Cif(self.data.clone())
    }

    fn next(&mut self) {
        self.chunk = self.chunks.next();
        self.reset_flags_for_chunk();
    }

    // TODO: a little bit convoluted
    fn skip_until_first_data_chunk(&mut self) {
        self.next();

        let mut is_stop = false;

        while !is_stop && self.chunk.is_some() {
            self.check_is_data_chunk();
            is_stop = self.local_flags.is_data_chunk;

            if is_stop {
                break;
            }

            self.next();
        }
    }

    fn check_is_empty(&mut self) -> Option<()> {
        (!self.chunk.unwrap().is_empty()).then_some(())
    }

    fn reset_flags_for_chunk(&mut self) {
        self.global_flags.is_new_line = self.local_flags.has_new_line_byte;
        self.local_flags = LocalFlags::default();
    }

    fn check_has_new_line_byte(&mut self) -> Option<()> {
        self.local_flags.has_new_line_byte = self.chunk.unwrap().last() == Some(&b'\n')
            || self.chunk.unwrap().last() == Some(&b'\r');

        Some(())
    }

    fn check_comment_byte(&mut self) -> Option<()> {
        if self.chunk.unwrap().starts_with(b"#") {
            self.global_flags.is_comment = true;
        }

        Some(())
    }

    fn check_comment_ends(&mut self) -> Option<()> {
        if self.global_flags.is_comment && self.local_flags.has_new_line_byte {
            self.global_flags.is_comment = false;
            return None;
        }

        Some(())
    }

    fn check_is_comment(&mut self) -> Option<()> {
        if self.global_flags.is_comment {
            return None;
        }

        Some(())
    }

    fn chunk_trim_ascii_end(&mut self) -> Option<()> {
        self.chunk = Some(self.chunk.unwrap().trim_ascii_end());

        log::debug!("{}", String::from_utf8_lossy(self.chunk.unwrap()));

        Some(())
    }

    fn check_is_start_or_end_of_multi_line_string(&mut self) -> Option<()> {
        if self.global_flags.is_new_line {
            self.global_flags.is_string_multiline ^= self.chunk.unwrap()[0] == b';';
        }

        Some(())
    }

    fn handle_if_multi_line_string(&mut self) -> Option<()> {
        if self.global_flags.is_string_multiline {
            let mut string_chunk = self.chunk.unwrap();

            string_chunk = string_chunk.strip_prefix(b";").unwrap_or(string_chunk);

            string_chunk = string_chunk
                .strip_suffix(b"\n")
                .unwrap_or(string_chunk)
                .strip_suffix(b"\r")
                .unwrap_or(string_chunk);

            self.string_multiline.extend(string_chunk);
            self.string_multiline.push(b' ');

            return None;
        }

        Some(())
    }

    fn handle_multi_line_string_end(&mut self) -> Option<()> {
        if !self.global_flags.is_string_multiline && !self.string_multiline.is_empty() {
            self.temp_data.values.push(
                String::from_utf8_lossy(&self.string_multiline)
                    .trim_ascii()
                    .to_string(),
            );

            self.string_multiline.clear();

            return None;
        }

        Some(())
    }

    fn check_is_string_with_spaces(&mut self) -> Option<()> {
        if self.chunk.unwrap().first() == Some(&b"'"[0])
            || self.chunk.unwrap().first() == Some(&b"\""[0])
        {
            self.global_flags.is_string_with_spaces = true;
        }

        Some(())
    }

    fn handle_if_string_with_spaces(&mut self) -> Option<()> {
        if self.global_flags.is_string_with_spaces {
            let string_chunk = self
                .chunk
                .unwrap()
                .iter()
                .filter(|&&byte| byte != b'\'' && byte != b'\"' && byte != b'\r' && byte != b'\n')
                .collect::<Vec<&u8>>();

            self.string_with_spaces.extend(string_chunk);
            self.string_with_spaces.push(b' ');

            match self.chunk.unwrap().last() == Some(&b'\'')
                || self.chunk.unwrap().last() == Some(&b'\"')
            {
                true => {
                    self.temp_data.values.push(
                        String::from_utf8_lossy(
                            self.string_with_spaces.strip_suffix(b" ").unwrap(),
                        )
                        .to_string(),
                    );

                    self.global_flags.is_string_with_spaces = false;

                    self.string_with_spaces.clear();

                    return None;
                }
                false => {
                    return None;
                }
            }
        }

        Some(())
    }

    fn check_loop_byte(&mut self) -> Option<()> {
        if self.chunk.unwrap().starts_with(b"loop_") {
            self.global_flags.is_loop = true;
            self.local_flags.is_loop_chunk = true;
            self.temp_data.names.clear();
            self.values_cleared_this_loop = 0;
        }

        Some(())
    }

    fn check_loop_end(&mut self) -> Option<()> {
        if self.global_flags.is_loop
            && self.local_flags.is_data_chunk
            && self.values_cleared_this_loop > 0
        {
            self.global_flags.is_loop = false;
            self.temp_data.names.clear();
            self.values_cleared_this_loop = 0;
        }

        Some(())
    }

    fn check_is_data_chunk(&mut self) -> Option<()> {
        if self.chunk.unwrap().starts_with(b"_") {
            self.local_flags.is_data_chunk = true;
        }

        Some(())
    }

    fn handle_if_data_chunk(&mut self) -> Option<()> {
        if self.local_flags.is_data_chunk {
            let string = String::from_utf8_lossy(self.chunk.unwrap()).to_string();

            self.temp_data.names.push(string);

            return None;
        }

        Some(())
    }

    fn handle_if_value_chunk(&mut self) -> Option<()> {
        if !self.local_flags.is_data_chunk && !self.local_flags.is_loop_chunk {
            let string = String::from_utf8_lossy(self.chunk.unwrap()).to_string();

            self.temp_data.values.push(string);
        }

        Some(())
    }

    fn update_data_from_temp_data(&mut self) -> Option<()> {
        if !self.temp_data.names.is_empty()
            && (self.temp_data.names.len() == self.temp_data.values.len())
        {
            for (name, value) in self
                .temp_data
                .names
                .iter()
                .zip(self.temp_data.values.iter())
            {
                self.data
                    .entry(name.clone())
                    .or_default()
                    .push(value.clone());
            }

            self.temp_data.values.clear();

            if self.global_flags.is_loop {
                self.values_cleared_this_loop += 1;
            }

            if !self.global_flags.is_loop || self.local_flags.is_loop_chunk {
                self.temp_data.names.clear();
            }
        }

        Some(())
    }
}

pub struct Cif(BTreeMap<String, Vec<String>>);

impl Cif {
    pub fn try_into_phase(self) -> anyhow::Result<crate::Phase> {
        crate::Phase::try_from(&self).context("Failed to parse phase")
    }
}

impl std::ops::Deref for Cif {
    type Target = BTreeMap<String, Vec<String>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub fn read_cif<'a>(bytes: &'a [u8]) -> Cif {
    let mut parser = Parser::<'a>::new(bytes);

    parser.parse()
}

fn split_input_into_chunks(bytes: &[u8]) -> impl Iterator<Item = &'_ [u8]> {
    bytes
        .split_inclusive(|&byte| byte == b'\n' || byte == b'\t' || byte == b' ')
        .filter(|&chunk| !chunk.is_empty() && chunk != [b' '] && chunk != [b'\t'])
}
