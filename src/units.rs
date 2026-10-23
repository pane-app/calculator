//! The units the calculator converts between, by what they measure.
//!
//! A unit's value in its dimension's base unit is `value * factor +
//! offset`; only the temperatures have an offset. The bases are the metre,
//! the kilogram, the litre, the square metre, the second, the metre per
//! second, the kelvin, the bit, the radian and the CSS pixel. Data sizes
//! follow the SI and IEC: `kB`, `MB` and `GB` are powers of 1000, `KiB`,
//! `MiB` and `GiB` powers of 1024, as Raycast's calculator has them.
//!
//! A unit is found by one of its `symbols`, compared exactly (`m` is a
//! metre, `Mb` a megabit, `MB` a megabyte), else by one of its `names`,
//! compared without case (`gb` is a gigabyte, `Miles` a mile).

use alloc::string::String;

/// What a unit measures. Only units of one dimension convert into each
/// other, add or subtract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dimension {
    Length,
    Mass,
    Volume,
    Area,
    Duration,
    Speed,
    Temperature,
    Data,
    Angle,
    Screen,
}

#[derive(Debug, PartialEq)]
pub struct Unit {
    /// How the calculator writes a value in this unit (`km`, `°F`).
    pub symbol: &'static str,
    pub dimension: Dimension,
    pub factor: f64,
    pub offset: f64,
    symbols: &'static [&'static str],
    names: &'static [&'static str],
}

impl Unit {
    /// `value` in this unit as a value in the dimension's base unit.
    pub fn to_base(&self, value: f64) -> f64 {
        value * self.factor + self.offset
    }

    /// A value in the dimension's base unit, in this unit.
    pub fn in_unit(&self, value: f64) -> f64 {
        (value - self.offset) / self.factor
    }
}

const fn unit(
    symbol: &'static str,
    dimension: Dimension,
    factor: f64,
    symbols: &'static [&'static str],
    names: &'static [&'static str],
) -> Unit {
    Unit {
        symbol,
        dimension,
        factor,
        offset: 0.0,
        symbols,
        names,
    }
}

use Dimension::*;

const INCH: f64 = 0.0254;
const FOOT: f64 = 0.3048;
const YARD: f64 = 0.9144;
const MILE: f64 = 1609.344;
const GALLON: f64 = 3.785_411_784;
const DAY: f64 = 86_400.0;

#[rustfmt::skip]
static UNITS: &[Unit] = &[
    // Length, in metres.
    unit("mm", Length, 0.001, &["mm"], &["millimeter", "millimeters", "millimetre", "millimetres"]),
    unit("cm", Length, 0.01, &["cm"], &["centimeter", "centimeters", "centimetre", "centimetres"]),
    unit("m", Length, 1.0, &["m"], &["meter", "meters", "metre", "metres"]),
    unit("km", Length, 1000.0, &["km"], &["kilometer", "kilometers", "kilometre", "kilometres"]),
    unit("nm", Length, 1e-9, &["nm"], &["nanometer", "nanometers", "nanometre", "nanometres"]),
    unit("µm", Length, 1e-6, &["µm", "um"], &["micrometer", "micrometers", "micrometre", "micrometres", "micron", "microns"]),
    unit("in", Length, INCH, &["in"], &["inch", "inches"]),
    unit("ft", Length, FOOT, &["ft"], &["foot", "feet"]),
    unit("yd", Length, YARD, &["yd"], &["yard", "yards", "yds"]),
    unit("mi", Length, MILE, &["mi"], &["mile", "miles"]),
    unit("nmi", Length, 1852.0, &["nmi"], &["nautical mile", "nautical miles"]),
    unit("ly", Length, 9_460_730_472_580_800.0, &["ly"], &["light year", "light years", "lightyear", "lightyears"]),
    // Mass, in kilograms.
    unit("mg", Mass, 1e-6, &["mg"], &["milligram", "milligrams", "milligramme", "milligrammes"]),
    unit("g", Mass, 0.001, &["g"], &["gram", "grams", "gramme", "grammes"]),
    unit("kg", Mass, 1.0, &["kg"], &["kilogram", "kilograms", "kilogramme", "kilogrammes", "kilo", "kilos"]),
    unit("t", Mass, 1000.0, &["t"], &["tonne", "tonnes", "metric ton", "metric tons"]),
    unit("oz", Mass, 0.028_349_523_125, &["oz"], &["ounce", "ounces"]),
    unit("lb", Mass, 0.453_592_37, &["lb", "lbs"], &["pound", "pounds"]),
    unit("st", Mass, 6.350_293_18, &["st"], &["stone", "stones"]),
    unit("ton", Mass, 907.184_74, &[], &["ton", "tons", "short ton", "short tons"]),
    // Volume, in litres.
    unit("ml", Volume, 0.001, &["ml", "mL"], &["milliliter", "milliliters", "millilitre", "millilitres"]),
    unit("cl", Volume, 0.01, &["cl", "cL"], &["centiliter", "centiliters", "centilitre", "centilitres"]),
    unit("dl", Volume, 0.1, &["dl", "dL"], &["deciliter", "deciliters", "decilitre", "decilitres"]),
    unit("l", Volume, 1.0, &["l", "L"], &["liter", "liters", "litre", "litres"]),
    unit("m³", Volume, 1000.0, &["m³", "m3"], &["cubic meter", "cubic meters", "cubic metre", "cubic metres"]),
    unit("cm³", Volume, 0.001, &["cm³", "cm3", "cc"], &["cubic centimeter", "cubic centimeters", "cubic centimetre", "cubic centimetres"]),
    unit("in³", Volume, INCH * INCH * INCH * 1000.0, &["in³", "in3"], &["cubic inch", "cubic inches"]),
    unit("ft³", Volume, FOOT * FOOT * FOOT * 1000.0, &["ft³", "ft3"], &["cubic foot", "cubic feet"]),
    unit("tsp", Volume, GALLON / 768.0, &["tsp"], &["teaspoon", "teaspoons"]),
    unit("tbsp", Volume, GALLON / 256.0, &["tbsp"], &["tablespoon", "tablespoons"]),
    unit("fl oz", Volume, GALLON / 128.0, &["floz"], &["fl oz", "fluid ounce", "fluid ounces"]),
    unit("cup", Volume, GALLON / 16.0, &[], &["cup", "cups"]),
    unit("pt", Volume, GALLON / 8.0, &["pt"], &["pint", "pints"]),
    unit("qt", Volume, GALLON / 4.0, &["qt"], &["quart", "quarts"]),
    unit("gal", Volume, GALLON, &["gal"], &["gallon", "gallons"]),
    // Area, in square metres.
    unit("mm²", Area, 1e-6, &["mm²", "mm2"], &["square millimeter", "square millimeters", "square millimetre", "square millimetres", "sq mm"]),
    unit("cm²", Area, 1e-4, &["cm²", "cm2"], &["square centimeter", "square centimeters", "square centimetre", "square centimetres", "sq cm"]),
    unit("m²", Area, 1.0, &["m²", "m2", "sqm"], &["square meter", "square meters", "square metre", "square metres", "sq m"]),
    unit("km²", Area, 1e6, &["km²", "km2", "sqkm"], &["square kilometer", "square kilometers", "square kilometre", "square kilometres", "sq km"]),
    unit("in²", Area, INCH * INCH, &["in²", "in2", "sqin"], &["square inch", "square inches", "sq in"]),
    unit("ft²", Area, FOOT * FOOT, &["ft²", "ft2", "sqft"], &["square foot", "square feet", "sq ft"]),
    unit("yd²", Area, YARD * YARD, &["yd²", "yd2", "sqyd"], &["square yard", "square yards", "sq yd"]),
    unit("mi²", Area, MILE * MILE, &["mi²", "mi2", "sqmi"], &["square mile", "square miles", "sq mi"]),
    unit("ha", Area, 10_000.0, &["ha"], &["hectare", "hectares"]),
    unit("ac", Area, 4_046.856_422_4, &["ac"], &["acre", "acres"]),
    // Duration, in seconds.
    unit("ms", Duration, 0.001, &["ms"], &["millisecond", "milliseconds", "msec", "msecs"]),
    unit("s", Duration, 1.0, &["s"], &["second", "seconds", "sec", "secs"]),
    unit("min", Duration, 60.0, &["min"], &["minute", "minutes", "mins"]),
    unit("h", Duration, 3600.0, &["h"], &["hour", "hours", "hr", "hrs"]),
    unit("d", Duration, DAY, &["d"], &["day", "days"]),
    unit("wk", Duration, 7.0 * DAY, &["wk"], &["week", "weeks", "wks"]),
    unit("mo", Duration, 30.436_875 * DAY, &["mo"], &["month", "months", "mos"]),
    unit("yr", Duration, 365.2425 * DAY, &["yr"], &["year", "years", "yrs"]),
    // Speed, in metres per second.
    unit("m/s", Speed, 1.0, &["m/s", "mps"], &["meters per second", "metres per second", "meter per second", "metre per second"]),
    unit("km/h", Speed, 1000.0 / 3600.0, &["km/h", "kmh", "kph"], &["kilometers per hour", "kilometres per hour", "kilometer per hour", "kilometre per hour"]),
    unit("mph", Speed, MILE / 3600.0, &["mph", "mi/h"], &["miles per hour", "mile per hour"]),
    unit("ft/s", Speed, FOOT, &["ft/s", "fps"], &["feet per second", "foot per second"]),
    unit("kn", Speed, 1852.0 / 3600.0, &["kn", "kt"], &["knot", "knots"]),
    // Temperature, in kelvins.
    Unit {
        symbol: "°C",
        dimension: Temperature,
        factor: 1.0,
        offset: 273.15,
        symbols: &["°C", "C", "c", "℃"],
        names: &["°c", "degc", "celsius", "centigrade"],
    },
    Unit {
        symbol: "°F",
        dimension: Temperature,
        factor: 5.0 / 9.0,
        offset: 459.67 * 5.0 / 9.0,
        symbols: &["°F", "F", "f", "℉"],
        names: &["°f", "degf", "fahrenheit"],
    },
    unit("K", Temperature, 1.0, &["K"], &["kelvin", "kelvins"]),
    // Data, in bits.
    unit("bit", Data, 1.0, &[], &["bit", "bits"]),
    unit("B", Data, 8.0, &["B"], &["byte", "bytes"]),
    unit("kB", Data, 8e3, &["kB", "KB"], &["kilobyte", "kilobytes", "kb"]),
    unit("MB", Data, 8e6, &["MB"], &["megabyte", "megabytes", "mb"]),
    unit("GB", Data, 8e9, &["GB"], &["gigabyte", "gigabytes", "gb"]),
    unit("TB", Data, 8e12, &["TB"], &["terabyte", "terabytes", "tb"]),
    unit("PB", Data, 8e15, &["PB"], &["petabyte", "petabytes", "pb"]),
    unit("KiB", Data, 8.0 * 1024.0, &["KiB"], &["kibibyte", "kibibytes", "kib"]),
    unit("MiB", Data, 8.0 * 1_048_576.0, &["MiB"], &["mebibyte", "mebibytes", "mib"]),
    unit("GiB", Data, 8.0 * 1_073_741_824.0, &["GiB"], &["gibibyte", "gibibytes", "gib"]),
    unit("TiB", Data, 8.0 * 1_099_511_627_776.0, &["TiB"], &["tebibyte", "tebibytes", "tib"]),
    unit("kbit", Data, 1e3, &["kbit", "Kb"], &["kilobit", "kilobits"]),
    unit("Mbit", Data, 1e6, &["Mbit", "Mb"], &["megabit", "megabits"]),
    unit("Gbit", Data, 1e9, &["Gbit", "Gb"], &["gigabit", "gigabits"]),
    // Angle, in radians.
    unit("rad", Angle, 1.0, &["rad"], &["radian", "radians", "rads"]),
    unit("°", Angle, core::f64::consts::PI / 180.0, &["°", "deg"], &["degree", "degrees", "degs"]),
    unit("turn", Angle, core::f64::consts::TAU, &[], &["turn", "turns", "revolution", "revolutions"]),
    // On screen, in CSS pixels; a rem or an em is 16 of them, as Raycast's
    // default base is.
    unit("px", Screen, 1.0, &["px"], &["pixel", "pixels"]),
    unit("pt", Screen, 4.0 / 3.0, &[], &["point", "points"]),
    unit("rem", Screen, 16.0, &["rem"], &["rems"]),
    unit("em", Screen, 16.0, &["em"], &["ems"]),
];

/// The unit `text` names, if any (see the module documentation).
pub fn find(text: &str) -> Option<&'static Unit> {
    if let Some(unit) = UNITS.iter().find(|unit| unit.symbols.contains(&text)) {
        return Some(unit);
    }
    let lower: String = text.to_lowercase();
    UNITS
        .iter()
        .find(|unit| unit.names.contains(&lower.as_str()))
}

/// Whether some unit's name starts with the word `text` and goes on: a
/// name of two words (`fl oz`, `light years`) is looked up word by word.
pub fn starts_a_name(text: &str) -> bool {
    let lower: String = text.to_lowercase();
    UNITS.iter().any(|unit| {
        unit.names.iter().any(|name| {
            name.strip_prefix(lower.as_str())
                .is_some_and(|rest| rest.starts_with(' '))
        })
    })
}
