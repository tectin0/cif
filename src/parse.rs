use std::{collections::BTreeMap, str::FromStr};

use anyhow::Context;
pub(crate) trait GetAndParse {
    fn get_and_parse_first<T: FromStr>(&self, key: &str) -> anyhow::Result<T>
    where
        <T as FromStr>::Err: Send,
        <T as FromStr>::Err: Sync,
        Result<T, <T as FromStr>::Err>: Context<T, <T as FromStr>::Err>,
        <T as FromStr>::Err: 'static;

    fn get_and_parse_all<T: FromStr>(&self, key: &str) -> anyhow::Result<Vec<T>>
    where
        <T as FromStr>::Err: Send,
        <T as FromStr>::Err: Sync,
        Result<T, <T as FromStr>::Err>: Context<T, <T as FromStr>::Err>,
        <T as FromStr>::Err: 'static;
}

impl GetAndParse for BTreeMap<String, Vec<String>> {
    fn get_and_parse_first<T: FromStr>(&self, key: &str) -> anyhow::Result<T>
    where
        <T as FromStr>::Err: Send,
        <T as FromStr>::Err: Sync,
        Result<T, <T as FromStr>::Err>: Context<T, <T as FromStr>::Err>,
        <T as FromStr>::Err: 'static,
    {
        self.get(key)
            .context(format!("Key: `{}` does not exist", key))?
            .first()
            .context(format!("Key: `{}` does not have a value", key))?
            .parse_without_uncertainty::<T>()
            .context(format!("Failed to parse value for key: `{}`", key))
    }

    fn get_and_parse_all<T: FromStr>(&self, key: &str) -> anyhow::Result<Vec<T>>
    where
        <T as FromStr>::Err: Send,
        <T as FromStr>::Err: Sync,
        Result<T, <T as FromStr>::Err>: Context<T, <T as FromStr>::Err>,
        <T as FromStr>::Err: 'static,
    {
        self.get(key)
            .context(format!("Key: `{}` does not exist", key))?
            .iter()
            .map(|value| {
                value
                    .parse_without_uncertainty::<T>()
                    .context(format!("Failed to parse value for key: `{}`", key))
            })
            .collect()
    }
}

trait ParseWithoutUncertainty {
    fn parse_without_uncertainty<T>(self) -> anyhow::Result<T>
    where
        T: FromStr,
        <T as FromStr>::Err: Send,
        <T as FromStr>::Err: Sync,
        Result<T, <T as FromStr>::Err>: Context<T, <T as FromStr>::Err>,
        <T as FromStr>::Err: 'static;
}

impl ParseWithoutUncertainty for &String {
    fn parse_without_uncertainty<T>(self) -> anyhow::Result<T>
    where
        T: FromStr,
        <T as FromStr>::Err: Send,
        <T as FromStr>::Err: Sync,
        Result<T, <T as FromStr>::Err>: Context<T, <T as FromStr>::Err>,
        <T as FromStr>::Err: 'static,
    {
        let stripped = self
            .as_bytes()
            .iter()
            .take_while(|&byte| byte != &b'(')
            .copied()
            .collect::<Vec<u8>>();

        String::from_utf8(stripped)
            .unwrap()
            .parse::<T>()
            .context("Failed to parse value")
    }
}
