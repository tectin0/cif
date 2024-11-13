use std::{collections::BTreeMap, string};

pub fn read_cif(bytes: Vec<u8>) -> BTreeMap<String, Vec<String>> {
    let chunks = bytes
        .split_inclusive(|&byte| byte == b'\n' || byte == b'\t' || byte == b' ')
        .filter(|chunk| !chunk.is_empty() && chunk != &[b' '] && chunk != &[b'\t']);

    let mut is_loop = false;

    let mut is_multi_line_string = false;
    let mut multi_line_string = Vec::<u8>::new();

    let mut is_string_with_spaces = false;
    let mut string_with_spaces = Vec::<u8>::new();

    let mut is_new_line = true;

    let mut data = BTreeMap::<String, Vec<String>>::new();

    let mut temp_data_names = Vec::<String>::new();
    let mut temp_data_values = Vec::<String>::new();

    let mut is_comment = false;

    let mut values_cleared_this_loop = 0;

    log::debug!("n: {}", b'\n');
    log::debug!("r: {}", b'\r');

    for chunk in chunks.skip(1) {
        if chunk.is_empty() {
            continue;
        }

        let mut data_value: Option<String> = None;

        let is_new_line_local = chunk.last() == Some(&b'\n') || chunk.last() == Some(&b'\r');

        if chunk.starts_with(b"#") {
            is_comment = true;
        }

        if is_new_line_local && is_comment {
            is_comment = false;
            is_new_line = is_new_line_local;
            log::debug!("{:?}", chunk);
            log::debug!("comment l: {}", String::from_utf8_lossy(chunk));
            continue;
        }

        if is_comment {
            log::debug!("{:?}", chunk);
            log::debug!("comment: {}", String::from_utf8_lossy(chunk));
            is_new_line = is_new_line_local;
            continue;
        }

        let mut chunk = chunk.trim_ascii_end();

        if chunk.is_empty() {
            is_new_line = is_new_line_local;
            continue;
        }

        log::debug!("{}", String::from_utf8_lossy(chunk));
        log::debug!(
            "nlen {} vlen {} is_loop {} is_new_line {} is_comment {} is_string_with_spaces {} is_multi_line_string {} values_cleared_this_loop {}",
            temp_data_names.len(),
            temp_data_values.len(),
            is_loop,
            is_new_line,
            is_comment,
            is_string_with_spaces,
            is_multi_line_string,
            values_cleared_this_loop
        );

        if is_new_line {
            is_multi_line_string ^= chunk[0] == b';';
        }

        if is_multi_line_string {
            let mut string_chunk = chunk;

            string_chunk = string_chunk.strip_prefix(b";").unwrap_or(&mut string_chunk);

            string_chunk = string_chunk
                .strip_suffix(b"\n")
                .unwrap_or(&mut string_chunk)
                .strip_suffix(b"\r")
                .unwrap_or(&mut string_chunk);

            multi_line_string.extend(string_chunk);
            multi_line_string.push(b' ');

            is_new_line = is_new_line_local;

            continue;
        }

        if !is_multi_line_string && !multi_line_string.is_empty() {
            data_value = Some(
                String::from_utf8_lossy(&multi_line_string)
                    .trim_ascii()
                    .to_string(),
            );

            multi_line_string.clear();
        }

        if chunk.first() == Some(&b"'"[0]) || chunk.first() == Some(&b"\""[0]) {
            is_string_with_spaces = true;
        }

        if is_string_with_spaces {
            let string_chunk = chunk
                .into_iter()
                .filter(|&&byte| byte != b'\'' && byte != b'\"' && byte != b'\r' && byte != b'\n')
                .collect::<Vec<&u8>>();

            string_with_spaces.extend(string_chunk);
            string_with_spaces.push(b' ');

            match chunk.last() == Some(&b'\'') || chunk.last() == Some(&b'\"') {
                true => {
                    data_value = Some(
                        String::from_utf8_lossy(&string_with_spaces.strip_suffix(b" ").unwrap())
                            .to_string(),
                    );

                    is_string_with_spaces = false;

                    string_with_spaces.clear();
                }
                false => {
                    is_new_line = is_new_line_local;

                    continue;
                }
            }
        }

        let is_value_none = data_value.is_none();

        if let Some(value) = data_value {
            temp_data_values.push(value);
        }

        let is_loop_local = chunk.starts_with(b"loop_");

        if is_loop_local {
            is_loop = true;
            temp_data_names.clear();
            values_cleared_this_loop = 0;
        }

        let is_data_name = chunk[0] == b'_';

        if is_loop && is_data_name && values_cleared_this_loop > 0 {
            is_loop = false;
            temp_data_names.clear();
            values_cleared_this_loop = 0;
        }

        if is_data_name {
            let string = String::from_utf8_lossy(&chunk).to_string();

            temp_data_names.push(string);

            is_new_line = is_new_line_local;

            continue;
        }

        if is_value_none && !is_data_name && !is_loop_local {
            let string = String::from_utf8_lossy(&chunk).to_string();

            temp_data_values.push(string);
        }

        if !temp_data_names.is_empty() && (temp_data_names.len() == temp_data_values.len()) {
            for (name, value) in temp_data_names.iter().zip(temp_data_values.iter()) {
                data.entry(name.clone()).or_default().push(value.clone());
            }

            temp_data_values.clear();

            if is_loop {
                values_cleared_this_loop += 1;
            }

            if !is_loop || is_loop_local {
                temp_data_names.clear();
            }
        }

        is_new_line = is_new_line_local;
    }

    data
}
