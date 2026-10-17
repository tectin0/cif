use std::{collections::BTreeMap, str::FromStr};

use anyhow::Context;
use crystallib::Measured;

/// A number in CIF's standard-uncertainty notation: the digits in parentheses are the esd in
/// units of the number's last digit, e.g. `5.4321(12)` has the esd 0.0012 and `1.23E-4(5)` the
/// esd 5E-6. A number without parentheses has no esd.
pub(crate) fn parse_measured(text: &str) -> anyhow::Result<Measured> {
    let Some((number, rest)) = text.split_once('(') else {
        let value = text
            .parse()
            .with_context(|| format!("`{text}` is not a number"))?;
        return Ok(Measured::new(value));
    };

    let value: f64 = number
        .parse()
        .with_context(|| format!("`{number}` is not a number"))?;
    let digits: u64 = rest
        .strip_suffix(')')
        .and_then(|digits| digits.parse().ok())
        .with_context(|| format!("`{text}` has no standard uncertainty in parentheses"))?;

    let (mantissa, exponent) = match number.find(['e', 'E']) {
        Some(index) => (
            &number[..index],
            number[index + 1..]
                .parse::<i32>()
                .with_context(|| format!("`{number}` has an invalid exponent"))?,
        ),
        None => (number, 0),
    };
    let decimals = mantissa
        .split_once('.')
        .map_or(0, |(_, fraction)| fraction.len() as i32);

    let shift = decimals - exponent;
    let esd = if shift >= 0 {
        digits as f64 / 10f64.powi(shift)
    } else {
        digits as f64 * 10f64.powi(-shift)
    };

    Ok(Measured {
        value,
        esd: Some(esd),
    })
}

pub(crate) trait GetAndParse {
    fn get_measured_first(&self, key: &str) -> anyhow::Result<Measured>;

    fn get_measured_all(&self, key: &str) -> anyhow::Result<Vec<Measured>>;

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
    fn get_measured_first(&self, key: &str) -> anyhow::Result<Measured> {
        let value = self
            .get(key)
            .context(format!("Key: `{}` does not exist", key))?
            .first()
            .context(format!("Key: `{}` does not have a value", key))?;

        parse_measured(value).context(format!("Failed to parse value for key: `{}`", key))
    }

    fn get_measured_all(&self, key: &str) -> anyhow::Result<Vec<Measured>> {
        self.get(key)
            .context(format!("Key: `{}` does not exist", key))?
            .iter()
            .map(|value| {
                parse_measured(value).context(format!("Failed to parse value for key: `{}`", key))
            })
            .collect()
    }

    fn get_and_parse_first<T: FromStr>(&self, key: &str) -> anyhow::Result<T>
    where
        <T as FromStr>::Err: Send,
        <T as FromStr>::Err: Sync,
        Result<T, <T as FromStr>::Err>: Context<T, <T as FromStr>::Err>,
        <T as FromStr>::Err: 'static,
    {
        let value = self
            .get(key)
            .context(format!("Key: `{}` does not exist", key))?
            .first()
            .context(format!("Key: `{}` does not have a value", key))?;

        value.parse_without_uncertainty::<T>().context(format!(
            "Failed to parse value `{}` for key: `{}`",
            value, key
        ))
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
                value.parse_without_uncertainty::<T>().context(format!(
                    "Failed to parse value `{}` for key: `{}`",
                    value, key
                ))
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

#[cfg(test)]
mod tests {
    use super::parse_measured;
    use crystallib::Measured;

    fn esd(text: &str) -> Option<f64> {
        parse_measured(text).unwrap().esd
    }

    #[test]
    fn the_digits_in_parentheses_are_the_esd_in_units_of_the_last_digit() {
        assert_eq!(
            parse_measured("5.4321(12)").unwrap(),
            Measured {
                value: 5.4321,
                esd: Some(0.0012),
            }
        );
        assert_eq!(esd("90.000(3)"), Some(0.003));
        assert_eq!(esd("90(1)"), Some(1.0));
        assert_eq!(esd("1.23E-4(5)"), Some(5e-6));
        assert_eq!(esd("12E3(4)"), Some(4000.0));
        assert_eq!(parse_measured("0.25").unwrap(), Measured::new(0.25));
    }

    #[test]
    fn malformed_numbers_are_errors() {
        assert!(parse_measured("?").is_err());
        assert!(parse_measured("1.0(").is_err());
        assert!(parse_measured("1.0(x)").is_err());
    }
}
