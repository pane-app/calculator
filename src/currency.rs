//! Currency conversions (pane-app/pane#475), after Raycast's calculator:
//!
//! - `100 usd in vnd`, `100 usd to vnd`, `100 USD as EUR`, `usd to vnd`
//!   (one of it), `100 usd` alone (into the user's own currency, the
//!   package's "Your currency" preference, else US dollars, and US dollars
//!   into euros);
//! - codes in any case: the world's currencies by their ISO 4217 codes and
//!   whatever else the rates hold, crypto included (`5 btc in eur`,
//!   `1 eth to usd`), three to five letters;
//! - symbols before or after the amount: `$100`, `€50`, `100€`, `£5`, `¥`,
//!   `₫`, `₹`, `₩`, `₽`, `₿` and the like, and `US$`, `A$`, `C$`, `R$` and
//!   other dollars written with their country;
//! - names, singular or plural: `dollars`, `euros`, `yen`, `dong`, `pounds`,
//!   `rupees`, `bitcoin` and others, and some with their country
//!   (`us dollars`, `japanese yen`);
//! - an amount the expression language computes (`1,000`, `2.5k`,
//!   `3 * 40`), and a code written against it (`100usd`).
//!
//! A word that is also a unit (`cup`, `ton`, `pound` alone) or a common
//! English word (`all`, `try`, `top`) is read as a currency only beside
//! another currency: `10 pounds` is a mass, `10 pounds in usd` a currency.
//!
//! An answer is written in the target's own digits: no decimals for the
//! yen, the dong or the won, three for the dinars, two for most, and up to
//! eight for crypto and metals; its thousands grouped as every other
//! answer's are (`2,583,200 VND`).

use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};

use crate::rates::Rates;
use crate::{Answer, Kind, expression, format as write, units};

/// A currency a query names.
#[derive(Clone, Debug, PartialEq)]
pub struct Named {
    /// Its code, in lower case.
    pub code: String,
    /// Whether it is a currency without the rates: one of the world's
    /// currencies, a well-known crypto, a symbol or a name. A code only the
    /// rates may hold (`aave`) is not.
    pub known: bool,
    /// Whether it may stand alone after an amount (`100 eur`), as a word
    /// that is also a unit or a common English word may not.
    pub alone: bool,
}

/// A conversion a query asks for.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub amount: f64,
    pub from: Named,
    /// `None` for an amount alone, converted into the user's currency.
    pub to: Option<Named>,
}

impl Request {
    /// Whether the query is a currency conversion even without the rates:
    /// every currency it names is known. Only then does an answer say the
    /// rates are not loaded yet.
    pub fn is_known(&self) -> bool {
        self.from.known && self.to.as_ref().is_none_or(|to| to.known)
    }
}

/// The ISO 4217 currencies, in lower case.
#[rustfmt::skip]
const WORLD: &[&str] = &[
    "aed", "afn", "all", "amd", "ang", "aoa", "ars", "aud", "awg", "azn", "bam", "bbd", "bdt",
    "bgn", "bhd", "bif", "bmd", "bnd", "bob", "brl", "bsd", "btn", "bwp", "byn", "bzd", "cad",
    "cdf", "chf", "clp", "cny", "cop", "crc", "cup", "cve", "czk", "djf", "dkk", "dop", "dzd",
    "egp", "ern", "etb", "eur", "fjd", "fkp", "gbp", "gel", "ghs", "gip", "gmd", "gnf", "gtq",
    "gyd", "hkd", "hnl", "htg", "huf", "idr", "ils", "inr", "iqd", "irr", "isk", "jmd", "jod",
    "jpy", "kes", "kgs", "khr", "kmf", "kpw", "krw", "kwd", "kyd", "kzt", "lak", "lbp", "lkr",
    "lrd", "lsl", "lyd", "mad", "mdl", "mga", "mkd", "mmk", "mnt", "mop", "mru", "mur", "mvr",
    "mwk", "mxn", "myr", "mzn", "nad", "ngn", "nio", "nok", "npr", "nzd", "omr", "pab", "pen",
    "pgk", "php", "pkr", "pln", "pyg", "qar", "ron", "rsd", "rub", "rwf", "sar", "sbd", "scr",
    "sdg", "sek", "sgd", "shp", "sle", "sos", "srd", "ssp", "stn", "syp", "szl", "thb", "tjs",
    "tmt", "tnd", "top", "try", "ttd", "twd", "tzs", "uah", "ugx", "usd", "uyu", "uzs", "ves",
    "vnd", "vuv", "wst", "xaf", "xcd", "xof", "xpf", "yer", "zar", "zmw", "zwl",
];

/// The world's currencies written with no decimals.
const NO_DECIMALS: &[&str] = &[
    "bif", "clp", "djf", "gnf", "isk", "jpy", "kmf", "krw", "pyg", "rwf", "ugx", "vnd", "vuv",
    "xaf", "xof", "xpf",
];

/// The world's currencies written with three decimals.
const THREE_DECIMALS: &[&str] = &["bhd", "iqd", "jod", "kwd", "lyd", "omr", "tnd"];

/// Crypto and metals known without the rates (the primary source holds
/// them; the fallback has none).
const OTHERS: &[&str] = &[
    "btc", "eth", "usdt", "usdc", "bnb", "xrp", "sol", "ada", "doge", "trx", "dot", "ltc", "bch",
    "xlm", "link", "xmr", "avax", "shib", "uni", "atom", "etc", "fil", "near", "xau", "xag", "xpt",
    "xpd",
];

/// Codes that are common words, read as currencies only beside another.
const WORDS: &[&str] = &[
    "all", "top", "try", "mad", "gel", "bob", "pen", "mop", "sos", "sol", "dot", "link", "near",
    "uni", "etc", "fil",
];

/// Words of the calculator's other languages that a code must not be.
const LANGUAGE: &[&str] = &[
    "hex", "bin", "oct", "dec", "sci", "mod", "plus", "minus", "times", "off", "what", "half",
    "sqrt", "cbrt", "sin", "cos", "tan", "asin", "acos", "atan", "sinh", "cosh", "tanh", "log",
    "exp", "abs", "round", "floor", "ceil", "fact", "tau", "phi", "now", "today", "ago", "next",
    "last", "until", "since", "time", "date", "from", "and", "base", "power", "into",
];

/// Symbols, as written (compared in lower case), and their codes.
const SYMBOLS: &[(&str, &str)] = &[
    ("us$", "usd"),
    ("a$", "aud"),
    ("au$", "aud"),
    ("c$", "cad"),
    ("ca$", "cad"),
    ("nz$", "nzd"),
    ("hk$", "hkd"),
    ("s$", "sgd"),
    ("r$", "brl"),
    ("mx$", "mxn"),
    ("$", "usd"),
    ("€", "eur"),
    ("£", "gbp"),
    ("¥", "jpy"),
    ("￥", "jpy"),
    ("円", "jpy"),
    ("元", "cny"),
    ("₫", "vnd"),
    ("đ", "vnd"),
    ("₹", "inr"),
    ("₩", "krw"),
    ("₽", "rub"),
    ("₺", "try"),
    ("₴", "uah"),
    ("₱", "php"),
    ("฿", "thb"),
    ("₪", "ils"),
    ("₦", "ngn"),
    ("₿", "btc"),
    ("zł", "pln"),
    ("kč", "czk"),
];

/// Names, in lower case, their codes, and whether they may stand alone.
#[rustfmt::skip]
const NAMES: &[(&str, &str, bool)] = &[
    ("dollar", "usd", true), ("dollars", "usd", true), ("buck", "usd", true), ("bucks", "usd", true),
    ("us dollar", "usd", true), ("us dollars", "usd", true),
    ("euro", "eur", true), ("euros", "eur", true),
    ("pound", "gbp", false), ("pounds", "gbp", false), ("sterling", "gbp", true),
    ("pound sterling", "gbp", true), ("pounds sterling", "gbp", true), ("quid", "gbp", true),
    ("british pound", "gbp", true), ("british pounds", "gbp", true),
    ("yen", "jpy", true), ("japanese yen", "jpy", true),
    ("dong", "vnd", true), ("vietnamese dong", "vnd", true), ("vnđ", "vnd", true),
    ("yuan", "cny", true), ("renminbi", "cny", true), ("rmb", "cny", true), ("chinese yuan", "cny", true),
    ("rupee", "inr", true), ("rupees", "inr", true), ("indian rupee", "inr", true), ("indian rupees", "inr", true),
    ("won", "krw", true), ("korean won", "krw", true), ("south korean won", "krw", true),
    ("ruble", "rub", true), ("rubles", "rub", true), ("rouble", "rub", true), ("roubles", "rub", true),
    ("baht", "thb", true), ("thai baht", "thb", true),
    ("franc", "chf", true), ("francs", "chf", true), ("swiss franc", "chf", true), ("swiss francs", "chf", true),
    ("peso", "mxn", true), ("pesos", "mxn", true), ("mexican peso", "mxn", true), ("mexican pesos", "mxn", true),
    ("real", "brl", true), ("reais", "brl", true), ("brazilian real", "brl", true), ("brazilian reais", "brl", true),
    ("rand", "zar", true), ("lira", "try", true), ("turkish lira", "try", true),
    ("zloty", "pln", true), ("zlotys", "pln", true), ("złoty", "pln", true),
    ("ringgit", "myr", true), ("rupiah", "idr", true),
    ("shekel", "ils", true), ("shekels", "ils", true),
    ("dirham", "aed", true), ("dirhams", "aed", true), ("riyal", "sar", true), ("riyals", "sar", true),
    ("hryvnia", "uah", true),
    ("canadian dollar", "cad", true), ("canadian dollars", "cad", true),
    ("australian dollar", "aud", true), ("australian dollars", "aud", true),
    ("new zealand dollar", "nzd", true), ("new zealand dollars", "nzd", true),
    ("hong kong dollar", "hkd", true), ("hong kong dollars", "hkd", true),
    ("singapore dollar", "sgd", true), ("singapore dollars", "sgd", true),
    ("taiwan dollar", "twd", true), ("taiwan dollars", "twd", true),
    ("bitcoin", "btc", true), ("bitcoins", "btc", true),
    ("ether", "eth", true), ("ethereum", "eth", true),
];

/// The words that convert an amount into the currency after them.
const INTO: &[&str] = &["in", "to", "as", "into"];

/// The currency `text` (one word or several, lower case) names, if any.
pub fn named(text: &str) -> Option<Named> {
    if let Some((_, code)) = SYMBOLS.iter().find(|(symbol, _)| *symbol == text) {
        return Some(Named {
            code: (*code).into(),
            known: true,
            alone: true,
        });
    }
    if let Some((_, code, alone)) = NAMES.iter().find(|(name, _, _)| *name == text) {
        return Some(Named {
            code: (*code).into(),
            known: true,
            alone: *alone,
        });
    }
    if !(3..=5).contains(&text.len()) || !text.bytes().all(|b| b.is_ascii_lowercase()) {
        return None;
    }
    if LANGUAGE.contains(&text) || units::find(text).is_some() {
        return None;
    }
    Some(Named {
        code: text.into(),
        known: WORLD.contains(&text) || OTHERS.contains(&text),
        alone: !WORDS.contains(&text),
    })
}

/// The conversion `query` asks for, if it asks for one (see the module
/// documentation); whether the rates hold its currencies is
/// [`convert`]'s to say.
pub fn request(query: &str) -> Option<Request> {
    let query = query.trim();
    let query = query.strip_suffix('=').unwrap_or(query).trim_end();
    if query.is_empty() || query.chars().count() > 256 {
        return None;
    }
    let lower = query.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    // The last `in`, `to`, `as` or `into` whose words after it name a
    // currency.
    if let Some(at) = (1..words.len().saturating_sub(1))
        .rev()
        .find(|&at| INTO.contains(&words[at]))
    {
        let to = named(&words[at + 1..].join(" "))?;
        let (amount, from) = source(&words[..at], true)?;
        return Some(Request {
            amount,
            from,
            to: Some(to),
        });
    }
    let (amount, from) = source(&words, false)?;
    if !from.alone {
        return None;
    }
    Some(Request {
        amount,
        from,
        to: None,
    })
}

/// The amount and the currency `words` name: a symbol before the amount,
/// or a currency after it (its own words, or written against it). With
/// `converted`, an amount may be left out (`usd to vnd`), meaning one.
fn source(words: &[&str], converted: bool) -> Option<(f64, Named)> {
    let text = words.join(" ");
    // A symbol first, longest first: `us$100`, `$100`, `€ 50`.
    let mut symbols: Vec<&(&str, &str)> = SYMBOLS.iter().collect();
    symbols.sort_by_key(|(symbol, _)| core::cmp::Reverse(symbol.len()));
    for (symbol, _) in symbols {
        if let Some(rest) = text.strip_prefix(symbol) {
            let amount = expression::number_of(rest)?;
            return Some((amount, named(symbol)?));
        }
    }
    // Its own words last: `100 us dollars`, `100 usd`.
    for count in (1..=3.min(words.len())).rev() {
        let (amount, currency) = words.split_at(words.len() - count);
        let Some(currency) = named(&currency.join(" ")) else {
            continue;
        };
        if amount.is_empty() {
            return (converted && currency.known).then_some((1.0, currency));
        }
        return Some((expression::number_of(&amount.join(" "))?, currency));
    }
    // Written against the amount: `100€`, `100usd`, `5btc`.
    let last = words.last()?;
    let at = last
        .char_indices()
        .find(|&(at, _)| {
            last[..at].ends_with(|c: char| c.is_ascii_digit())
                && last[at..].chars().all(|c| !c.is_ascii_digit())
        })?
        .0;
    let currency = named(&last[at..]).filter(|currency| currency.known)?;
    let mut amount: Vec<&str> = words[..words.len() - 1].to_vec();
    amount.push(&last[..at]);
    Some((expression::number_of(&amount.join(" "))?, currency))
}

/// How many decimals an amount of `code` is written with: `None` for crypto
/// and metals, written with up to eight.
fn decimals(code: &str) -> Option<usize> {
    if NO_DECIMALS.contains(&code) {
        Some(0)
    } else if THREE_DECIMALS.contains(&code) {
        Some(3)
    } else if WORLD.contains(&code) {
        Some(2)
    } else {
        None
    }
}

/// `value` of `code` as the answer shows it and as it copies it plain.
fn amount(value: f64, code: &str) -> (String, String) {
    let shown = |grouped: bool| match decimals(code) {
        // Too small for its decimals, or too large to write out: as other
        // answers are written.
        Some(places) if value.abs() < 1e15 && (value == 0.0 || round(value, places) != 0.0) => {
            let text = format!("{value:.places$}");
            let text = if text.starts_with('-') && round(value, places) == 0.0 {
                text[1..].to_string()
            } else {
                text
            };
            if grouped { write::group(&text) } else { text }
        }
        Some(_) => write::plain(value, grouped),
        None => write::number(value, 8, grouped),
    };
    let upper = code.to_uppercase();
    (format!("{} {upper}", shown(true)), shown(false))
}

/// `value` rounded to `places` decimals.
fn round(value: f64, places: usize) -> f64 {
    let scale = libm::pow(10.0, places as f64);
    libm::round(value * scale) / scale
}

/// The currency an amount alone is converted into: the user's own, `home`
/// (lower case), unless the amount is in it, then US dollars, or euros for
/// an amount in US dollars.
fn into(from: &str, home: &str) -> String {
    if from != home {
        home.into()
    } else if from != "usd" {
        "usd".into()
    } else {
        "eur".into()
    }
}

/// The answer `request` has with `rates`, `home` the user's currency (lower
/// case); `None` when the rates do not hold one of its currencies.
pub fn convert(request: &Request, rates: &Rates, home: &str) -> Option<Answer> {
    let from = request.from.code.as_str();
    let to = match &request.to {
        Some(to) => to.code.clone(),
        None => into(from, home),
    };
    let value = request.amount / rates.get(from)? * rates.get(&to)?;
    if !value.is_finite() {
        return None;
    }
    let (text, plain) = amount(value, &to);
    Some(Answer {
        text,
        plain,
        kind: Kind::Currency,
        swatch: None,
        copies: Vec::new(),
    })
}

/// The user's currency as the "Your currency" preference names it (a code,
/// a symbol or a name, in any case), US dollars if it names none.
pub fn home(preference: Option<&str>) -> String {
    preference
        .map(|text| text.trim().to_lowercase())
        .and_then(|text| named(&text))
        .filter(|currency| currency.known)
        .map_or_else(|| "usd".into(), |currency| currency.code)
}

/// The section a currency answer is listed under, naming the day its rates
/// are for ("Currency · rates from 23 Oct", with the year when it is not
/// `today`'s) and crediting their source where its terms ask (ExchangeRate-API's:
/// "… by Exchange Rate API"); `today` in days since the Unix epoch.
pub fn section(rates: &Rates, today: i64) -> String {
    format!("Currency · {}", note(rates, today))
}

/// What a currency answer says of its rates: "rates from 23 Oct", and "by
/// Exchange Rate API" after it for the fallback's (see [`section`]).
pub fn note(rates: &Rates, today: i64) -> String {
    let day = described_day(rates.day, today);
    match rates.credit() {
        Some(credit) => format!("rates from {day} by {credit}"),
        None => format!("rates from {day}"),
    }
}

/// `day` as "23 Oct", with its year when it is not `today`'s year.
pub fn described_day(day: i64, today: i64) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let (year, month, date) = crate::dates::civil_from_days(day);
    let (this_year, _, _) = crate::dates::civil_from_days(today);
    let month = MONTHS[(month as usize).saturating_sub(1).min(11)];
    if year == this_year {
        format!("{date} {month}")
    } else {
        format!("{date} {month} {year}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rates::tests::primary_rates;
    use crate::rates::{Rates, Source};

    /// The currencies `query` converts between and its amount.
    fn asked(query: &str) -> Option<(f64, String, Option<String>)> {
        request(query).map(|request| {
            (
                request.amount,
                request.from.code,
                request.to.map(|to| to.code),
            )
        })
    }

    fn pair(amount: f64, from: &str, to: &str) -> Option<(f64, String, Option<String>)> {
        Some((amount, from.into(), Some(to.into())))
    }

    fn alone(amount: f64, from: &str) -> Option<(f64, String, Option<String>)> {
        Some((amount, from.into(), None))
    }

    /// What `query` answers with the primary test rates, the user's
    /// currency the dong.
    fn shown(query: &str) -> String {
        let request = request(query).unwrap_or_else(|| panic!("{query} asks for nothing"));
        convert(&request, &primary_rates(0), "vnd")
            .unwrap_or_else(|| panic!("{query} has no answer"))
            .text
    }

    #[test]
    fn codes_convert_with_in_to_and_as() {
        assert_eq!(asked("100 usd in vnd"), pair(100.0, "usd", "vnd"));
        assert_eq!(asked("100 usd to vnd"), pair(100.0, "usd", "vnd"));
        assert_eq!(asked("100 USD as EUR"), pair(100.0, "usd", "eur"));
        assert_eq!(asked("100 Usd into Vnd ="), pair(100.0, "usd", "vnd"));
        assert_eq!(asked("  5 btc in eur "), pair(5.0, "btc", "eur"));
        assert_eq!(asked("1 ETH to usd"), pair(1.0, "eth", "usd"));
        assert_eq!(asked("usd to vnd"), pair(1.0, "usd", "vnd"));
        assert_eq!(asked("100 aave in usd"), pair(100.0, "aave", "usd"));
    }

    #[test]
    fn symbols_go_before_or_after_the_amount() {
        assert_eq!(asked("$100 in eur"), pair(100.0, "usd", "eur"));
        assert_eq!(asked("€50 to usd"), pair(50.0, "eur", "usd"));
        assert_eq!(asked("€ 50 to $"), pair(50.0, "eur", "usd"));
        assert_eq!(asked("50€ in £"), pair(50.0, "eur", "gbp"));
        assert_eq!(asked("50 € in ¥"), pair(50.0, "eur", "jpy"));
        assert_eq!(asked("US$20 in ₫"), pair(20.0, "usd", "vnd"));
        assert_eq!(asked("A$20 in C$"), pair(20.0, "aud", "cad"));
        assert_eq!(asked("₿0.5 in usd"), pair(0.5, "btc", "usd"));
        assert_eq!(asked("100usd in eur"), pair(100.0, "usd", "eur"));
        assert_eq!(asked("$100"), alone(100.0, "usd"));
    }

    #[test]
    fn names_are_read_singular_or_plural() {
        assert_eq!(asked("100 dollars in euros"), pair(100.0, "usd", "eur"));
        assert_eq!(asked("100 Euro to Dollar"), pair(100.0, "eur", "usd"));
        assert_eq!(asked("1000 yen to dong"), pair(1000.0, "jpy", "vnd"));
        assert_eq!(
            asked("20 us dollars in vietnamese dong"),
            pair(20.0, "usd", "vnd")
        );
        assert_eq!(asked("10 pounds in usd"), pair(10.0, "gbp", "usd"));
        assert_eq!(asked("2 bitcoin in dollars"), pair(2.0, "btc", "usd"));
        assert_eq!(asked("100 dong"), alone(100.0, "vnd"));
    }

    #[test]
    fn an_amount_is_computed_as_the_language_computes() {
        assert_eq!(asked("1,000 usd in vnd"), pair(1000.0, "usd", "vnd"));
        assert_eq!(asked("2.5k eur in usd"), pair(2500.0, "eur", "usd"));
        assert_eq!(asked("3 * 40 usd in eur"), pair(120.0, "usd", "eur"));
        assert_eq!(asked("$(10 + 5) in eur"), pair(15.0, "usd", "eur"));
        assert_eq!(asked("100 eur"), alone(100.0, "eur"));
    }

    #[test]
    fn units_dates_and_words_are_left_to_the_rest_of_the_calculator() {
        for query in [
            "10 pounds",
            "10 pounds in kg",
            "5 cup in ml",
            "2 cups",
            "1 ton",
            "255 in hex",
            "5 km in miles",
            "time in tokyo",
            "5pm london in new york",
            "now + 3 days",
            "10 all",
            "usd",
            "dollars",
            "2 + 2",
            "100 in",
            "100 usd in kg",
            "0xff",
            "10k",
        ] {
            assert_eq!(request(query), None, "{query}");
        }
    }

    #[test]
    fn answers_use_the_targets_digits_and_group_thousands() {
        assert_eq!(shown("100 usd in vnd"), "2,583,200 VND");
        assert_eq!(shown("100 usd in eur"), "86.00 EUR");
        assert_eq!(shown("€50 to usd"), "58.14 USD");
        assert_eq!(shown("1000 yen in dollars"), "6.56 USD");
        assert_eq!(shown("10 usd in kwd"), "3.070 KWD");
        assert_eq!(shown("1 btc in usd"), "100,000.00 USD");
        assert_eq!(shown("100 usd in btc"), "0.001 BTC");
        assert_eq!(shown("1 usd in eth"), "0.00025 ETH");
        assert_eq!(
            shown("1 vnd in usd"),
            "0.0000387117 USD",
            "too small for cents"
        );
        assert_eq!(shown("100 eur"), "3,003,721 VND");
        assert_eq!(shown("0 usd in eur"), "0.00 EUR");
    }

    #[test]
    fn the_unformatted_answer_is_the_number_alone() {
        let request = request("100 usd in vnd").unwrap();
        let answer = convert(&request, &primary_rates(0), "usd").unwrap();
        assert_eq!(answer.plain, "2583200");
        assert_eq!(answer.kind, Kind::Currency);
    }

    #[test]
    fn an_amount_alone_goes_into_the_users_currency() {
        let rates = primary_rates(0);
        let into = |query: &str, home: &str| {
            convert(&request(query).unwrap(), &rates, home)
                .unwrap()
                .text
        };
        assert_eq!(into("100 eur", "vnd"), "3,003,721 VND");
        assert_eq!(into("100 eur", "usd"), "116.28 USD");
        assert_eq!(
            into("100 usd", "usd"),
            "86.00 EUR",
            "its own currency: into euros"
        );
        assert_eq!(into("100 eur", "eur"), "116.28 USD");
        assert_eq!(home(Some(" VND ")), "vnd");
        assert_eq!(home(Some("dong")), "vnd");
        assert_eq!(home(Some("€")), "eur");
        assert_eq!(home(Some("nonsense")), "usd");
        assert_eq!(home(None), "usd");
    }

    #[test]
    fn currencies_the_rates_lack_have_no_answer() {
        let fallback = Rates::from_fallback(&crate::rates::tests::fallback_body(), 0).unwrap();
        assert_eq!(fallback.source, Source::Fallback);
        let request = request("1 btc in usd").unwrap();
        assert_eq!(convert(&request, &fallback, "usd"), None);
        let request = super::request("5 dogs").unwrap();
        assert!(!request.is_known());
        assert_eq!(convert(&request, &fallback, "usd"), None);
        assert!(super::request("100 usd in vnd").unwrap().is_known());
    }

    #[test]
    fn the_section_names_the_rates_day_and_credits_the_fallback() {
        let today = crate::dates::days_from_civil(2026, 10, 24);
        let primary = primary_rates(0);
        assert_eq!(section(&primary, today), "Currency · rates from 23 Oct");
        let fallback = Rates::from_fallback(&crate::rates::tests::fallback_body(), 0).unwrap();
        assert_eq!(
            section(&fallback, today),
            "Currency · rates from 22 Oct by Exchange Rate API"
        );
        let next_year = crate::dates::days_from_civil(2027, 1, 2);
        assert_eq!(
            section(&primary, next_year),
            "Currency · rates from 23 Oct 2026"
        );
    }
}
