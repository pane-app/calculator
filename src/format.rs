//! How the calculator writes numbers: at most 15 significant digits, no
//! trailing zeros, thousands grouped with commas (`1,234,567.5`) and
//! scientific notation (`1.5e20`) for values from a quadrillion up or
//! below a millionth, as Raycast's calculator shows them.

use alloc::{format, string::String};

/// `value` as the calculator shows it, with at most `decimals` decimal
/// places, its thousands grouped when `grouped`.
pub fn number(value: f64, decimals: usize, grouped: bool) -> String {
    let magnitude = value.abs();
    if magnitude == 0.0 {
        return "0".into();
    }
    if !(1e-6..1e15).contains(&magnitude) {
        let text = format!("{value:.14e}");
        let (mantissa, exponent) = text.split_once('e').expect("LowerExp writes an e");
        return format!("{}e{exponent}", trimmed(mantissa));
    }
    let mut digits = 1;
    let mut bound = 10.0;
    while magnitude >= bound {
        digits += 1;
        bound *= 10.0;
    }
    let decimals = 15usize.saturating_sub(digits).min(decimals);
    let text = format!("{value:.decimals$}");
    let text = trimmed(&text);
    // -0.0000000000001 rounds to "-0".
    if text == "-0" {
        return "0".into();
    }
    if grouped { group(text) } else { text.into() }
}

/// A plain number's answer: 10 decimal places at most.
pub fn plain(value: f64, grouped: bool) -> String {
    number(value, 10, grouped)
}

/// A converted quantity's number: 5 decimal places at most, as Raycast
/// shows `3 teaspoon in ml` as 14.78676.
pub fn quantity(value: f64, grouped: bool) -> String {
    number(value, 5, grouped)
}

/// `number` without trailing zeros after its decimal point, or the point.
fn trimmed(number: &str) -> &str {
    if number.contains('.') {
        number.trim_end_matches('0').trim_end_matches('.')
    } else {
        number
    }
}

/// `text`, a number in plain notation, with commas between its whole
/// part's groups of three digits.
pub fn group(text: &str) -> String {
    let (sign, rest) = match text.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", text),
    };
    let (whole, fraction) = match rest.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (rest, None),
    };
    let mut out = String::from(sign);
    for (index, digit) in whole.chars().enumerate() {
        if index > 0 && (whole.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    if let Some(fraction) = fraction {
        out.push('.');
        out.push_str(fraction);
    }
    out
}

/// A whole number `value` written in base `radix` (2, 8 or 16) with its
/// prefix (`0b`, `0o`, `0x`), hexadecimal digits in capitals; `None` for a
/// value that is not a whole number from 0 to 2^53.
pub fn in_base(value: f64, radix: u32) -> Option<String> {
    if !(0.0..=9_007_199_254_740_992.0).contains(&value) || value != libm::trunc(value) {
        return None;
    }
    let mut whole = value as u64;
    let prefix = match radix {
        2 => "0b",
        8 => "0o",
        16 => "0x",
        _ => return None,
    };
    let mut digits = alloc::vec::Vec::new();
    loop {
        let digit = (whole % u64::from(radix)) as u32;
        digits.push(char::from_digit(digit, radix)?.to_ascii_uppercase());
        whole /= u64::from(radix);
        if whole == 0 {
            break;
        }
    }
    let mut out = String::from(prefix);
    out.extend(digits.iter().rev());
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_numbers_group_their_thousands() {
        assert_eq!(plain(4096.0, true), "4,096");
        assert_eq!(plain(1_234_567.5, true), "1,234,567.5");
        assert_eq!(plain(-1000.0, true), "-1,000");
        assert_eq!(plain(999.0, true), "999");
        assert_eq!(plain(1_234_567.5, false), "1234567.5");
    }

    #[test]
    fn answers_keep_at_most_fifteen_significant_digits() {
        assert_eq!(plain(1.0 / 3.0, true), "0.3333333333");
        assert_eq!(plain(0.1 + 0.2, true), "0.3");
        assert_eq!(plain(2e20, true), "2e20");
        assert_eq!(plain(1.5e-7, true), "1.5e-7");
        assert_eq!(plain(-1e-13, true), "-1e-13");
    }

    #[test]
    fn quantities_keep_five_decimal_places() {
        assert_eq!(quantity(14.786_764_782, true), "14.78676");
        assert_eq!(quantity(3.106_855_961_186_67, true), "3.10686");
    }

    #[test]
    fn bases_carry_their_prefix() {
        assert_eq!(in_base(255.0, 16).as_deref(), Some("0xFF"));
        assert_eq!(in_base(10.0, 2).as_deref(), Some("0b1010"));
        assert_eq!(in_base(8.0, 8).as_deref(), Some("0o10"));
        assert_eq!(in_base(0.0, 16).as_deref(), Some("0x0"));
        assert_eq!(in_base(1.5, 16), None);
        assert_eq!(in_base(-1.0, 16), None);
    }
}
