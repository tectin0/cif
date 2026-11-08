//! CIF's numbers: a value with an optional standard uncertainty in parentheses, in units of the
//! value's last digit (`5.4321(12)` is 5.4321 ± 0.0012, `1.23E-4(5)` is 1.23E-4 ± 5E-6).

/// The value and standard uncertainty of `text`, or `None` if it isn't a CIF number.
pub fn parse(text: &str) -> Option<(f64, Option<f64>)> {
    let Some((number, rest)) = text.split_once('(') else {
        return text.parse().ok().map(|value| (value, None));
    };

    let value: f64 = number.parse().ok()?;
    let digits: u64 = rest.strip_suffix(')')?.parse().ok()?;

    let (mantissa, exponent) = match number.find(['e', 'E']) {
        Some(index) => (&number[..index], number[index + 1..].parse::<i32>().ok()?),
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

    Some((value, Some(esd)))
}

/// `value` as a CIF number. With a positive, finite `esd`, the uncertainty is written with two
/// digits when its leading digit is 1, else with one (so it lies within 2–19 in units of the last
/// digit), and the value is rounded to the same digit. Without one, the value is written in full.
pub fn format(value: f64, esd: Option<f64>) -> String {
    let Some(esd) = esd.filter(|esd| esd.is_finite() && *esd > 0.0) else {
        return format!("{value}");
    };

    // The number of decimals that puts the uncertainty within 2–19 units of the last digit.
    let mut decimals = (-(esd / 19.5).log10()).ceil() as i32;
    let units = |decimals: i32| (esd * 10f64.powi(decimals)).round();
    if units(decimals) >= 20.0 {
        decimals -= 1;
    } else if units(decimals) < 2.0 {
        decimals += 1;
    }
    let units = units(decimals) as u64;

    if decimals >= 0 {
        format!("{value:.prec$}({units})", prec = decimals as usize)
    } else {
        let scale = 10f64.powi(-decimals);
        let rounded = (value / scale).round() * scale;
        format!("{rounded:.0}({})", units * scale as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_number_reads_with_its_uncertainty_in_units_of_its_last_digit() {
        assert_eq!(parse("5.4321(12)"), Some((5.4321, Some(0.0012))));
        assert_eq!(parse("1.23E-4(5)"), Some((1.23e-4, Some(5e-6))));
        assert_eq!(parse("120(3)"), Some((120.0, Some(3.0))));
        assert_eq!(parse("0.5"), Some((0.5, None)));
        assert_eq!(parse("?"), None);
        assert_eq!(parse("1.2(x)"), None);
    }

    #[test]
    fn a_number_writes_its_uncertainty_with_one_or_two_digits() {
        assert_eq!(format(5.43214, Some(0.0012)), "5.4321(12)");
        assert_eq!(format(5.43214, Some(0.0004)), "5.4321(4)");
        assert_eq!(format(3.52279, None), "3.52279");
        assert_eq!(format(12345.0, Some(50.0)), "12350(50)");
        assert_eq!(format(1.0, Some(0.0)), "1");
    }

    #[test]
    fn a_written_number_reads_back_within_its_rounding() {
        for (value, esd) in [(5.43214, 0.0012), (0.000123, 0.0000041), (98.7, 1.9)] {
            let (read, read_esd) = parse(&format(value, Some(esd))).unwrap();
            let read_esd = read_esd.unwrap();
            assert!((read - value).abs() <= read_esd, "{value} ± {esd}");
            assert!((read_esd - esd).abs() <= esd * 0.1, "{value} ± {esd}");
        }
    }
}
