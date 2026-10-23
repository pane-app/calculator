//! The calculator's colours (#196, first written in Pane's own guests and
//! brought here): the forms a query names a colour in, and the forms the
//! answer copies it as.
//!
//! A query names a colour as `#RGB`, `#RGBA`, `#RRGGBB` or `#RRGGBBAA`
//! (hex digits, either case, each short digit standing for two of it), or
//! as a function: `rgb(r, g, b)` and `rgba(r, g, b, a)` with channels
//! 0-255 and an alpha 0-1, `hsl(h, s%, l%)` and `hsla(h, s%, l%, a)` with
//! a hue in degrees, and `oklch(l c h)` with a lightness 0-1, a chroma
//! and a hue in degrees, and an optional `/ a` alpha. The legacy
//! functions' arguments are comma-separated and `oklch`'s
//! space-separated, as CSS writes them; an argument out of range is
//! clamped, as CSS clips it. A number is digits and one decimal point, no
//! sign, and a colour name is not a colour: `red` is a word, as Raycast
//! leaves names out.
//!
//! The answer is the colour as uppercase hex (`#RRGGBB`, or `#RRGGBBAA`
//! while it is not fully opaque), shown with a swatch, and the Actions
//! panel offers it as hex, RGB, HSL and OKLCH. The OKLab matrices are
//! Björn Ottosson's.

use alloc::{format, string::String, vec::Vec};

/// A colour: sRGB channels 0-255 (rounded when the form it was parsed
/// from does not name whole ones) and an alpha 0-1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Colour {
    red: u8,
    green: u8,
    blue: u8,
    alpha: f64,
}

/// The colour the query names, if it is one.
pub fn parse(query: &str) -> Option<Colour> {
    let query = query.trim();
    if let Some(hex) = query.strip_prefix('#') {
        return parse_hex(hex);
    }
    let form = ["rgb(", "rgba(", "hsl(", "hsla(", "oklch("]
        .into_iter()
        .find_map(|form| Some((form, query.strip_prefix(form)?)));
    let (form, rest) = form?;
    let rest = rest.strip_suffix(')')?;
    let parts: Vec<&str> = rest.split(',').map(str::trim).collect();
    match form {
        "rgb(" | "rgba(" => match parts.as_slice() {
            [red, green, blue] => Some(Colour {
                red: byte(number(red)?),
                green: byte(number(green)?),
                blue: byte(number(blue)?),
                alpha: 1.0,
            }),
            [red, green, blue, alpha] => Some(Colour {
                red: byte(number(red)?),
                green: byte(number(green)?),
                blue: byte(number(blue)?),
                alpha: number(alpha)?.clamp(0.0, 1.0),
            }),
            _ => None,
        },
        "hsl(" | "hsla(" => match parts.as_slice() {
            [hue, saturation, lightness] => Some(from_hsl(
                number(hue)?,
                percent(saturation)?,
                percent(lightness)?,
                1.0,
            )),
            [hue, saturation, lightness, alpha] => Some(from_hsl(
                number(hue)?,
                percent(saturation)?,
                percent(lightness)?,
                number(alpha)?.clamp(0.0, 1.0),
            )),
            _ => None,
        },
        // `oklch` separates its arguments with spaces, not commas.
        _ if rest.contains(',') => None,
        _ => {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            match parts.as_slice() {
                [lightness, chroma, hue] => Some(from_oklch(
                    number(lightness)?.clamp(0.0, 1.0),
                    number(chroma)?.max(0.0),
                    number(hue)?,
                    1.0,
                )),
                [lightness, chroma, hue, "/", alpha] => Some(from_oklch(
                    number(lightness)?.clamp(0.0, 1.0),
                    number(chroma)?.max(0.0),
                    number(hue)?,
                    number(alpha)?.clamp(0.0, 1.0),
                )),
                _ => None,
            }
        }
    }
}

/// `hex`, the digits after `#`, as a colour; `None` unless it holds 3, 4,
/// 6 or 8 hex digits.
fn parse_hex(hex: &str) -> Option<Colour> {
    let digits: Vec<u8> = hex
        .chars()
        .map(|c| c.to_digit(16).map(|digit| digit as u8))
        .collect::<Option<Vec<u8>>>()?;
    // Each digit of the short forms stands for two of it: '#3aa' is
    // '#33aaaa', '#3aab' '#33aaaabb'.
    let digits: Vec<u8> = match digits.len() {
        3 | 4 => digits.iter().flat_map(|digit| [*digit, *digit]).collect(),
        6 | 8 => digits,
        _ => return None,
    };
    let channel = |digits: &[u8]| digits[0] * 16 + digits[1];
    Some(Colour {
        red: channel(&digits[0..2]),
        green: channel(&digits[2..4]),
        blue: channel(&digits[4..6]),
        alpha: if digits.len() == 8 {
            f64::from(channel(&digits[6..8])) / 255.0
        } else {
            1.0
        },
    })
}

/// `text` as a number: digits with at most one decimal point.
fn number(text: &str) -> Option<f64> {
    let text = text.trim();
    let digits = text.bytes().filter(u8::is_ascii_digit).count();
    let points = text.bytes().filter(|&b| b == b'.').count();
    if digits == 0 || points > 1 || digits + points != text.len() {
        return None;
    }
    text.parse().ok()
}

/// `text` as the number of a percentage, requiring its `%`.
fn percent(text: &str) -> Option<f64> {
    number(text.strip_suffix('%')?)
}

/// `value` as a channel, clamped to 0-255 and rounded.
fn byte(value: f64) -> u8 {
    libm::round(value.clamp(0.0, 255.0)) as u8
}

/// `value` as an alpha's byte.
fn alpha_byte(alpha: f64) -> u8 {
    libm::round(alpha.clamp(0.0, 1.0) * 255.0) as u8
}

/// The remainder of `x / m`, in [0, m) for `m > 0`.
fn remainder(x: f64, m: f64) -> f64 {
    let r = x % m;
    if r < 0.0 { r + m } else { r }
}

/// `hue` in degrees, wrapped into [0, 360).
fn degrees(hue: f64) -> f64 {
    remainder(hue, 360.0)
}

/// The colour of the HSL values (hue in degrees, saturation and
/// lightness as percentages).
fn from_hsl(hue: f64, saturation: f64, lightness: f64, alpha: f64) -> Colour {
    let (saturation, lightness) = (
        saturation.clamp(0.0, 100.0) / 100.0,
        lightness.clamp(0.0, 100.0) / 100.0,
    );
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let sector = degrees(hue) / 60.0;
    let secondary = chroma * (1.0 - (remainder(sector, 2.0) - 1.0).abs());
    let [red, green, blue] = match sector as u32 {
        0 => [chroma, secondary, 0.0],
        1 => [secondary, chroma, 0.0],
        2 => [0.0, chroma, secondary],
        3 => [0.0, secondary, chroma],
        4 => [secondary, 0.0, chroma],
        _ => [chroma, 0.0, secondary],
    };
    let lift = lightness - chroma / 2.0;
    Colour {
        red: byte((red + lift) * 255.0),
        green: byte((green + lift) * 255.0),
        blue: byte((blue + lift) * 255.0),
        alpha,
    }
}

/// The colour of the OKLCH values (lightness 0-1, chroma, hue in
/// degrees), clipped to sRGB's gamut.
fn from_oklch(lightness: f64, chroma: f64, hue: f64, alpha: f64) -> Colour {
    let radians = degrees(hue).to_radians();
    let (a, b) = (chroma * libm::cos(radians), chroma * libm::sin(radians));
    let cube = |x: f64| x * x * x;
    // OKLab to linear sRGB, through the cone-like LMS response.
    let long = cube(lightness + 0.396_337_777_4 * a + 0.215_803_757_3 * b);
    let medium = cube(lightness - 0.105_561_345_8 * a - 0.063_854_172_8 * b);
    let short = cube(lightness - 0.089_484_177_5 * a - 1.291_485_548_0 * b);
    let channel = |value: f64| {
        // Linear light to a gamma-encoded channel, clipped to [0, 1]:
        // a value outside sRGB's gamut becomes its nearest channel.
        let encoded = if value <= 0.0 {
            0.0
        } else {
            1.055 * libm::pow(value, 1.0 / 2.4) - 0.055
        };
        byte(encoded * 255.0)
    };
    Colour {
        red: channel(4.076_741_662_1 * long - 3.307_711_591_3 * medium + 0.230_969_929_2 * short),
        green: channel(
            -1.268_438_004_6 * long + 2.609_757_401_1 * medium - 0.341_399_396_5 * short,
        ),
        blue: channel(-0.004_196_086_3 * long - 0.703_418_614_7 * medium + 1.707_614_701_0 * short),
        alpha,
    }
}

impl Colour {
    /// The colour as uppercase hex: `#RRGGBB` while it is fully opaque,
    /// `#RRGGBBAA` otherwise.
    pub fn hex(self) -> String {
        let alpha = alpha_byte(self.alpha);
        if alpha == 255 {
            format!("#{:02X}{:02X}{:02X}", self.red, self.green, self.blue)
        } else {
            format!(
                "#{:02X}{:02X}{:02X}{:02X}",
                self.red, self.green, self.blue, alpha
            )
        }
    }

    /// The colour as CSS's `rgb()`.
    pub fn rgb(self) -> String {
        format!("rgb({}, {}, {})", self.red, self.green, self.blue)
    }

    /// The colour as CSS's `hsl()` (the alpha is only in the hex form).
    pub fn hsl(self) -> String {
        let (red, green, blue) = (
            f64::from(self.red) / 255.0,
            f64::from(self.green) / 255.0,
            f64::from(self.blue) / 255.0,
        );
        let max = red.max(green).max(blue);
        let min = red.min(green).min(blue);
        let lightness = (max + min) / 2.0;
        let (hue, saturation) = if max == min {
            (0.0, 0.0)
        } else {
            let difference = max - min;
            let saturation = difference / (1.0 - (2.0 * lightness - 1.0).abs());
            let hue = if max == red {
                (green - blue) / difference * 60.0
            } else if max == green {
                ((blue - red) / difference + 2.0) * 60.0
            } else {
                ((red - green) / difference + 4.0) * 60.0
            };
            (hue, saturation)
        };
        format!(
            "hsl({}, {}%, {}%)",
            // Modulo again, so a hue a hair under 360 rounds to 0, not 360.
            libm::round(degrees(hue)) as i64 % 360,
            libm::round(saturation * 100.0) as i64,
            libm::round(lightness * 100.0) as i64,
        )
    }

    /// The colour as CSS's `oklch()`: lightness and chroma with three
    /// decimals at most, and a hue in degrees — none of it when the
    /// colour has no chroma.
    pub fn oklch(self) -> String {
        let channel = |value: f64| {
            // A gamma-encoded channel to linear light.
            if value <= 0.04045 {
                value / 12.92
            } else {
                libm::pow((value + 0.055) / 1.055, 2.4)
            }
        };
        let [red, green, blue] = [
            channel(f64::from(self.red) / 255.0),
            channel(f64::from(self.green) / 255.0),
            channel(f64::from(self.blue) / 255.0),
        ];
        // Linear sRGB to OKLab, through the cone-like LMS response.
        let long = 0.412_221_470_8 * red + 0.536_332_536_3 * green + 0.051_445_992_9 * blue;
        let medium = 0.211_903_498_2 * red + 0.680_699_545_1 * green + 0.107_396_956_6 * blue;
        let short = 0.088_302_461_9 * red + 0.281_718_837_6 * green + 0.629_978_700_5 * blue;
        let (long, medium, short) = (libm::cbrt(long), libm::cbrt(medium), libm::cbrt(short));
        let lightness = 0.210_454_255_3 * long + 0.793_617_785_0 * medium - 0.004_072_046_8 * short;
        let a = 1.977_998_495_1 * long - 2.428_592_205_0 * medium + 0.450_593_709_9 * short;
        let b = 0.025_904_037_1 * long + 0.782_771_766_2 * medium - 0.808_675_766_0 * short;
        let chroma = libm::sqrt(a * a + b * b);
        let hue = libm::atan2(b, a).to_degrees();
        let chroma = rounded(chroma);
        let hue = if chroma == "0" {
            0
        } else {
            // Modulo again, so a hue a hair under 360 rounds to 0.
            libm::round(degrees(hue)) as i64 % 360
        };
        format!("oklch({} {} {})", rounded(lightness), chroma, hue)
    }
}

/// `value` as a component of an `oklch()` copy: three decimals at most,
/// without trailing zeros.
fn rounded(value: f64) -> String {
    let text = format!("{value:.3}");
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "-0" {
        "0".into()
    } else {
        trimmed.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(query: &str) -> Option<String> {
        parse(query).map(Colour::hex)
    }

    #[test]
    fn hex_colours_in_every_length() {
        assert_eq!(hex("#3aa").as_deref(), Some("#33AAAA"));
        assert_eq!(hex("#3AAB").as_deref(), Some("#33AAAABB"));
        assert_eq!(hex("#ff8800").as_deref(), Some("#FF8800"));
        assert_eq!(hex("#FF880080").as_deref(), Some("#FF880080"));
        assert_eq!(hex("#ff880"), None);
        assert_eq!(hex("#gg0000"), None);
    }

    #[test]
    fn css_functions_name_colours() {
        assert_eq!(hex("rgb(255, 136, 0)").as_deref(), Some("#FF8800"));
        assert_eq!(hex("rgba(255, 136, 0, 0.5)").as_deref(), Some("#FF880080"));
        assert_eq!(hex("rgb(300, 0, 0)").as_deref(), Some("#FF0000"));
        assert_eq!(hex("hsl(120, 100%, 50%)").as_deref(), Some("#00FF00"));
        assert_eq!(hex("hsla(240, 100%, 50%, 1)").as_deref(), Some("#0000FF"));
        assert_eq!(hex("oklch(0.628 0.258 29.23)").as_deref(), Some("#FF0000"));
        assert_eq!(hex("oklch(1 0 0)").as_deref(), Some("#FFFFFF"));
        assert_eq!(hex("oklch(0.5 0 0 / 0.5)").as_deref(), Some("#63636380"));
        assert_eq!(hex("rgb(a, b, c)"), None);
        assert_eq!(hex("rgb( , 0, 0)"), None);
        assert_eq!(hex("oklch(0.5, 0, 0)"), None);
        assert_eq!(hex("red"), None);
    }

    #[test]
    fn a_colour_copies_in_each_form() {
        let orange = parse("#FF8800").unwrap();
        assert_eq!(orange.rgb(), "rgb(255, 136, 0)");
        assert_eq!(orange.hsl(), "hsl(32, 100%, 50%)");
        assert_eq!(parse("#FF0000").unwrap().oklch(), "oklch(0.628 0.258 29)");
        assert_eq!(parse("#808080").unwrap().oklch(), "oklch(0.6 0 0)");
        assert_eq!(parse("#000").unwrap().hsl(), "hsl(0, 0%, 0%)");
    }
}
