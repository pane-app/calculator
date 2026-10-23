//! The calculator's expression language, after what Raycast's calculator
//! reads (pane-app/pane#472):
//!
//! - numbers: `12`, `3.5`, `.5`, `1,000,000` (commas between groups of
//!   three), `1e3`, `10k`, `2M` and `3bn` (thousand, million, billion),
//!   `0xff`, `0b1010`, `0o17`;
//! - `+`, `-` (or `−`, `–`), `*` (or `×`, `x`, `·`), `/` (or `÷`), `^` (or
//!   `**`) and `mod`, and the words `plus`, `minus`, `times`,
//!   `multiplied by`, `divided by`, `power` and `to the power of`, with the
//!   usual precedence: a power first, right to left, then the products,
//!   then the sums;
//! - a leading sign (`-3`, `2 * -(1 + 1)`; `-2^2` is `-(2^2)`), parentheses,
//!   and multiplication without a sign before a parenthesis, a constant or
//!   a function (`2pi`, `3(4 + 5)`, `(1 + 2)(3 + 4)`, `2 sqrt 9`);
//! - `5!`, `5²`, `5³`, `5 squared`, `5 cubed`, `√16`, `∛27`,
//!   `square root of 16`, `cube root of 27`, `half of 30`;
//! - the constants `pi` (`π`), `e`, `tau` (`τ`) and `phi` (`φ`), and the
//!   functions `sqrt`, `cbrt`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`,
//!   `sinh`, `cosh`, `tanh`, `ln`, `log` (base 10), `log2`, `log10`, `exp`,
//!   `abs`, `round`, `floor`, `ceil` and `fact`, with or without
//!   parentheses (`sqrt 16`); angles are radians unless a unit says
//!   otherwise (`sin(90°)`, `cos 180 degrees`);
//! - percentages: `20% of 50`, `50 + 10%`, `50 - 10%`, `50 * 10%`,
//!   `10% off 50`, `5% on 30`, `10 is what % of 50`, `what % of 50 is 10`,
//!   `10 as a % of 50`, `10 is 20% of what`;
//! - units (see [`crate::units`]) after a number, added and subtracted
//!   across their dimension (`5 km + 300 m`, `5 ft 3 in`), and converted
//!   with `in`, `to`, `as` or `into` (`5 km in miles`, `100 f to c`,
//!   `1 GB in MB`, `2 rem in px`);
//! - conversions of a number to `hex`, `binary`, `octal`, `decimal`,
//!   `scientific` or `%` (`255 in hex`, `0xff in decimal`, `0.35 as %`);
//! - spaces anywhere between those, and a trailing `=`.
//!
//! A query longer than 256 characters, or nesting parentheses more than 64
//! deep, is not understood, so that no query can exhaust the calculator's
//! stack.
//!
//! An expression has an answer only if it calculates something: a number
//! alone, like `42`, `(5)`, `pi`, `10%` or `5 km`, is not a calculation,
//! but `0xff` is (it answers 255). Arithmetic is IEEE double precision;
//! answers are shown with at most 15 significant digits.

use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};

use crate::format as write;
use crate::units::{self, Dimension, Unit};
use crate::{Answer, Kind, Outcome};

#[derive(Clone, Debug, PartialEq)]
enum Token {
    /// A number, and whether it was written with a base's prefix.
    Number(f64, bool),
    Word(String),
    Plus,
    Minus,
    Times,
    Divide,
    Power,
    Percent,
    Bang,
    Open,
    Close,
    /// `√` (2) or `∛` (3).
    Root(u8),
    /// `²` (2) or `³` (3).
    Superscript(u8),
}

/// Why no value was computed.
#[derive(Debug)]
enum Failure {
    Incomplete,
    Invalid,
}

/// A value while it is computed: a number, the unit it is in, and whether
/// it is a percentage (`n` is then the fraction: 10% is 0.1).
#[derive(Clone, Copy, Debug)]
struct Value {
    n: f64,
    unit: Option<&'static Unit>,
    percent: bool,
}

impl Value {
    fn plain(n: f64) -> Value {
        Value {
            n,
            unit: None,
            percent: false,
        }
    }

    fn is_plain(&self) -> bool {
        self.unit.is_none() && !self.percent
    }
}

/// What a conversion asks for.
#[derive(Clone, Copy, Debug)]
enum Target {
    Unit(&'static Unit),
    Base(u32),
    Percent,
    Scientific,
}

/// The longest query evaluated, in characters.
const MAX_LENGTH: usize = 256;

/// The deepest nesting of parentheses evaluated.
const MAX_DEPTH: u32 = 64;

const CONSTANTS: &[(&str, f64)] = &[
    ("pi", core::f64::consts::PI),
    ("π", core::f64::consts::PI),
    ("e", core::f64::consts::E),
    ("tau", core::f64::consts::TAU),
    ("τ", core::f64::consts::TAU),
    ("phi", 1.618_033_988_749_895),
    ("φ", 1.618_033_988_749_895),
];

const FUNCTIONS: &[&str] = &[
    "sqrt", "cbrt", "sin", "cos", "tan", "asin", "acos", "atan", "arcsin", "arccos", "arctan",
    "sinh", "cosh", "tanh", "ln", "log", "log2", "log10", "exp", "abs", "round", "floor", "ceil",
    "fact",
];

/// Evaluates `query` (see the module documentation for the language).
pub fn evaluate(query: &str) -> Outcome {
    let query = query.trim();
    let query = query.strip_suffix('=').unwrap_or(query).trim_end();
    if query.chars().count() > MAX_LENGTH {
        return Outcome::Invalid;
    }
    let Some(tokens) = tokens(query) else {
        return Outcome::Invalid;
    };
    if tokens.is_empty() {
        return Outcome::Invalid;
    }
    if let Some(outcome) = percent_question(&tokens) {
        return outcome;
    }
    // A conversion: an `in`, `to`, `as` or `into` outside parentheses
    // whose words after it name a target, the last tried first.
    let mut depth = 0i32;
    let mut splits = Vec::new();
    for (at, token) in tokens.iter().enumerate() {
        match token {
            Token::Open => depth += 1,
            Token::Close => depth -= 1,
            Token::Word(word)
                if depth == 0
                    && at > 0
                    && matches!(lower(word).as_str(), "in" | "to" | "as" | "into") =>
            {
                splits.push(at);
            }
            _ => {}
        }
    }
    let mut first_failure = None;
    for &at in splits.iter().rev() {
        let Some(target) = target(&tokens[at + 1..]) else {
            continue;
        };
        match computed(&tokens[..at]) {
            Ok((value, _, _)) => return converted(value, target),
            Err(failure) => {
                first_failure.get_or_insert(failure);
            }
        }
    }
    match computed(&tokens) {
        Ok((value, operations, prefixed)) => {
            if operations == 0 && !(prefixed && value.is_plain()) {
                Outcome::NotACalculation
            } else {
                answer(value)
            }
        }
        // A conversion of a value that failed says why that value did.
        Err(failure) => outcome(first_failure.unwrap_or(failure)),
    }
}

/// The plain number `text` computes, if it computes one with no unit and
/// no percentage: the amount of a currency conversion (`100`, `1,000`,
/// `2.5k`, `3 * 40`), which [`crate::currency`] reads with the language's
/// own numbers and arithmetic.
pub fn number_of(text: &str) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > MAX_LENGTH {
        return None;
    }
    let tokens = tokens(text)?;
    if tokens.is_empty() {
        return None;
    }
    let (value, _, _) = computed(&tokens).ok()?;
    (value.is_plain() && value.n.is_finite()).then_some(value.n)
}

fn outcome(failure: Failure) -> Outcome {
    match failure {
        Failure::Incomplete => Outcome::Incomplete,
        Failure::Invalid => Outcome::Invalid,
    }
}

/// The value of `tokens` as a whole, how many operations it applied and
/// whether a number was written with a base's prefix.
fn computed(tokens: &[Token]) -> Result<(Value, u32, bool), Failure> {
    let mut parser = Parser {
        tokens,
        next: 0,
        operations: 0,
        depth: 0,
        prefixed: false,
    };
    let value = parser.sum()?;
    if parser.next != tokens.len() {
        // A closing parenthesis too many, two values in a row, or words
        // the language has not.
        return Err(Failure::Invalid);
    }
    if !value.n.is_finite() {
        return Err(Failure::Invalid);
    }
    Ok((value, parser.operations, parser.prefixed))
}

/// The answer a computed value is.
fn answer(value: Value) -> Outcome {
    if !value.n.is_finite() {
        return Outcome::Invalid;
    }
    let (text, plain) = if value.percent {
        percentage(value.n)
    } else if let Some(unit) = value.unit {
        quantity(value.n, unit)
    } else {
        (write::plain(value.n, true), write::plain(value.n, false))
    };
    Outcome::Answer(Answer {
        text,
        plain,
        kind: Kind::Number,
        swatch: None,
        copies: Vec::new(),
    })
}

/// The fraction `n` as a percentage, grouped and plain.
fn percentage(n: f64) -> (String, String) {
    let n = n * 100.0;
    (
        format!("{}%", write::plain(n, true)),
        format!("{}%", write::plain(n, false)),
    )
}

/// `n` of `unit`, grouped and plain.
fn quantity(n: f64, unit: &Unit) -> (String, String) {
    (
        format!("{} {}", write::quantity(n, true), unit.symbol),
        format!("{} {}", write::quantity(n, false), unit.symbol),
    )
}

/// `value` converted as `target` asks.
fn converted(value: Value, target: Target) -> Outcome {
    let answer = |(text, plain): (String, String)| {
        Outcome::Answer(Answer {
            text,
            plain,
            kind: Kind::Number,
            swatch: None,
            copies: Vec::new(),
        })
    };
    match target {
        Target::Unit(unit) => {
            let Some(from) = value.unit else {
                return Outcome::Invalid;
            };
            if from.dimension != unit.dimension || value.percent {
                return Outcome::Invalid;
            }
            let n = unit.in_unit(from.to_base(value.n));
            if !n.is_finite() {
                return Outcome::Invalid;
            }
            answer(quantity(n, unit))
        }
        Target::Base(10) if value.is_plain() => {
            let text = write::plain(value.n, false);
            answer((text.clone(), text))
        }
        Target::Base(radix) if value.is_plain() => match write::in_base(value.n, radix) {
            Some(text) => answer((text.clone(), text)),
            None => Outcome::Invalid,
        },
        Target::Percent if value.unit.is_none() => answer(percentage(value.n)),
        Target::Scientific if value.is_plain() => {
            let text = scientific(value.n);
            answer((text.clone(), text))
        }
        _ => Outcome::Invalid,
    }
}

/// `n` in scientific notation, its mantissa's trailing zeros dropped.
fn scientific(n: f64) -> String {
    let text = format!("{n:.14e}");
    let (mantissa, exponent) = text.split_once('e').expect("LowerExp writes an e");
    let mantissa = if mantissa.contains('.') {
        mantissa.trim_end_matches('0').trim_end_matches('.')
    } else {
        mantissa
    };
    format!("{mantissa}e{exponent}")
}

/// The conversion `tokens` name, if they name one and nothing else.
fn target(tokens: &[Token]) -> Option<Target> {
    let words: Option<Vec<String>> = tokens
        .iter()
        .map(|token| match token {
            Token::Word(word) => Some(lower(word)),
            Token::Number(n, false) => Some(n.to_string()),
            Token::Percent => Some("%".into()),
            _ => None,
        })
        .collect();
    let named = match words?.join(" ").as_str() {
        "hex" | "hexadecimal" | "base 16" => Some(Target::Base(16)),
        "binary" | "bin" | "base 2" => Some(Target::Base(2)),
        "octal" | "oct" | "base 8" => Some(Target::Base(8)),
        "decimal" | "dec" | "base 10" => Some(Target::Base(10)),
        "%" | "percent" | "a percentage" | "percentage" | "a %" => Some(Target::Percent),
        "sci" | "scientific" | "scientific notation" => Some(Target::Scientific),
        _ => None,
    };
    if named.is_some() {
        return named;
    }
    let parser = Parser {
        tokens,
        next: 0,
        operations: 0,
        depth: 0,
        prefixed: false,
    };
    match parser.unit_at(0) {
        Some((unit, length)) if length == tokens.len() => Some(Target::Unit(unit)),
        _ => None,
    }
}

/// The percentage questions: `A is what % of B`, `what % of B is A`,
/// `A as a % of B` (A over B), and `A is P% of what` (A over P).
fn percent_question(tokens: &[Token]) -> Option<Outcome> {
    let word =
        |at: usize, text: &str| matches!(tokens.get(at), Some(Token::Word(w)) if lower(w) == text);
    let percent = |at: usize| {
        matches!(tokens.get(at), Some(Token::Percent))
            || matches!(tokens.get(at), Some(Token::Word(w)) if lower(w) == "percent")
    };
    let ratio = |a: &[Token], b: &[Token]| -> Outcome {
        let (Ok((a, _, _)), Ok((b, _, _))) = (computed(a), computed(b)) else {
            return Outcome::Invalid;
        };
        let (a, b) = match (a.unit, b.unit) {
            (None, None) => (a.n, b.n),
            (Some(x), Some(y)) if x.dimension == y.dimension => (x.to_base(a.n), y.to_base(b.n)),
            _ => return Outcome::Invalid,
        };
        if b == 0.0 {
            return Outcome::Invalid;
        }
        answer(Value {
            n: a / b,
            unit: None,
            percent: true,
        })
    };
    // what % of B is A
    if word(0, "what") && percent(1) && word(2, "of") {
        let is = (3..tokens.len()).find(|&at| word(at, "is"))?;
        return Some(ratio(&tokens[is + 1..], &tokens[3..is]));
    }
    let last = tokens.len() - 1;
    for at in 1..tokens.len() {
        // A is what % of B
        if word(at, "is") && word(at + 1, "what") && percent(at + 2) && word(at + 3, "of") {
            return Some(ratio(&tokens[..at], &tokens[at + 4..]));
        }
        // A as a % of B, A as % of B
        if word(at, "as") {
            let skip = usize::from(word(at + 1, "a"));
            if percent(at + 1 + skip) && word(at + 2 + skip, "of") {
                return Some(ratio(&tokens[..at], &tokens[at + 3 + skip..]));
            }
        }
        // A is P% of what
        if word(at, "is") && last >= at + 3 && word(last, "what") && word(last - 1, "of") {
            let (Ok((a, _, _)), Ok((p, _, _))) =
                (computed(&tokens[..at]), computed(&tokens[at + 1..last - 1]))
            else {
                return Some(Outcome::Invalid);
            };
            if !p.percent || p.n == 0.0 || p.unit.is_some() {
                return Some(Outcome::Invalid);
            }
            return Some(answer(Value {
                n: a.n / p.n,
                unit: a.unit,
                percent: false,
            }));
        }
    }
    None
}

fn lower(word: &str) -> String {
    word.to_lowercase()
}

/// `query` as tokens; `None` if it has anything outside the language.
fn tokens(query: &str) -> Option<Vec<Token>> {
    let chars: Vec<char> = query.chars().collect();
    let mut tokens = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        if c.is_whitespace() {
            at += 1;
            continue;
        }
        let single = match c {
            '+' => Some(Token::Plus),
            '-' | '\u{2212}' | '\u{2013}' => Some(Token::Minus),
            '*' if chars.get(at + 1) == Some(&'*') => {
                at += 1;
                Some(Token::Power)
            }
            '*' | '×' | '·' | '⋅' => Some(Token::Times),
            '/' | '÷' | '∕' => Some(Token::Divide),
            '^' => Some(Token::Power),
            '%' => Some(Token::Percent),
            '!' => Some(Token::Bang),
            '(' | '[' => Some(Token::Open),
            ')' | ']' => Some(Token::Close),
            '√' => Some(Token::Root(2)),
            '∛' => Some(Token::Root(3)),
            '²' => Some(Token::Superscript(2)),
            '³' => Some(Token::Superscript(3)),
            _ => None,
        };
        if let Some(token) = single {
            tokens.push(token);
            at += 1;
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && chars.get(at + 1).is_some_and(char::is_ascii_digit)) {
            let (token, next) = number(&chars, at)?;
            tokens.push(token);
            at = next;
            continue;
        }
        if is_word(c) {
            let (word, next) = word(&chars, at);
            tokens.push(Token::Word(word));
            at = next;
            continue;
        }
        return None;
    }
    Some(tokens)
}

fn is_word(c: char) -> bool {
    c.is_alphabetic() || matches!(c, '°' | '℃' | '℉')
}

/// The word starting at `start`, and where it ends: `log2` and `log10`,
/// and a unit written with its power (`m²`, `cm3`) or as a quotient
/// (`km/h`, `m/s`), are one word.
fn word(chars: &[char], start: usize) -> (String, usize) {
    let mut at = start;
    while at < chars.len() && is_word(chars[at]) {
        at += 1;
    }
    let mut word: String = chars[start..at].iter().collect();
    let digit_at = |at: usize| chars.get(at).is_some_and(char::is_ascii_digit);
    if word.eq_ignore_ascii_case("log") {
        for digits in ["10", "2"] {
            let end = at + digits.len();
            let follows: String = chars[at..end.min(chars.len())].iter().collect();
            if follows == digits && !digit_at(end) {
                word.push_str(digits);
                return (word, end);
            }
        }
    }
    if let Some(&power @ ('²' | '³' | '2' | '3')) = chars.get(at)
        && !digit_at(at + 1)
    {
        let mut unit = word.clone();
        unit.push(power);
        if units::find(&unit).is_some() {
            return (unit, at + 1);
        }
    }
    if chars.get(at) == Some(&'/') {
        let mut end = at + 1;
        while end < chars.len() && is_word(chars[end]) {
            end += 1;
        }
        let unit: String = chars[start..end].iter().collect();
        if end > at + 1 && units::find(&unit).is_some() {
            return (unit, end);
        }
    }
    (word, at)
}

/// The number starting at `start`, and where it ends.
fn number(chars: &[char], start: usize) -> Option<(Token, usize)> {
    let digit_at = |at: usize| chars.get(at).is_some_and(char::is_ascii_digit);
    let alphanumeric_at = |at: usize| chars.get(at).is_some_and(|c| c.is_alphanumeric());
    // 0x, 0b and 0o: at least one digit of the base, and no letter or
    // digit after them (`0B` alone is zero bytes).
    if chars[start] == '0' {
        let radix = match chars.get(start + 1) {
            Some('x' | 'X') => 16,
            Some('b' | 'B') => 2,
            Some('o' | 'O') => 8,
            _ => 0,
        };
        if radix != 0 {
            let mut at = start + 2;
            let mut value: u64 = 0;
            while let Some(digit) = chars.get(at).and_then(|c| c.to_digit(radix)) {
                value = value
                    .checked_mul(u64::from(radix))?
                    .checked_add(u64::from(digit))?;
                at += 1;
            }
            if at > start + 2 && !alphanumeric_at(at) {
                return Some((Token::Number(value as f64, true), at));
            }
        }
    }
    let mut text = String::new();
    let mut at = start;
    while digit_at(at) {
        text.push(chars[at]);
        at += 1;
    }
    // Commas between groups of three digits.
    if (1..=3).contains(&text.len()) {
        while chars.get(at) == Some(&',') && (1..=3).all(|k| digit_at(at + k)) && !digit_at(at + 4)
        {
            text.extend(&chars[at + 1..at + 4]);
            at += 4;
        }
    }
    if chars.get(at) == Some(&'.') && digit_at(at + 1) {
        text.push('.');
        at += 1;
        while digit_at(at) {
            text.push(chars[at]);
            at += 1;
        }
    }
    // An exponent: e3, E-7, e+2; `2e` alone is 2 times e.
    if matches!(chars.get(at), Some('e' | 'E')) {
        let sign = matches!(chars.get(at + 1), Some('+' | '-'));
        let first = at + 1 + usize::from(sign);
        if digit_at(first) {
            let mut end = first;
            while digit_at(end) {
                end += 1;
            }
            if !alphanumeric_at(end) {
                text.extend(&chars[at..end]);
                at = end;
            }
        }
    }
    let mut value: f64 = text.parse().ok()?;
    // 10k, 2M and 3bn, written against the number.
    let scale = match (chars.get(at), chars.get(at + 1)) {
        (Some('k' | 'K'), _) if !alphanumeric_at(at + 1) => Some((1e3, 1)),
        (Some('M'), _) if !alphanumeric_at(at + 1) => Some((1e6, 1)),
        (Some('b'), Some('n')) if !alphanumeric_at(at + 2) => Some((1e9, 2)),
        _ => None,
    };
    if let Some((factor, length)) = scale {
        value *= factor;
        at += length;
    }
    Some((Token::Number(value, false), at))
}

/// A recursive descent over the tokens, one method per precedence level.
struct Parser<'a> {
    tokens: &'a [Token],
    next: usize,
    /// How many operations were applied.
    operations: u32,
    /// How many parentheses enclose the next token.
    depth: u32,
    /// Whether a number was written with a base's prefix.
    prefixed: bool,
}

/// How two values are combined at the level of products.
#[derive(Clone, Copy, PartialEq)]
enum Product {
    Times,
    Divide,
    Modulo,
    Of,
    Off,
    On,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.next)
    }

    /// The word at `next + ahead`, in lower case.
    fn word(&self, ahead: usize) -> Option<String> {
        match self.tokens.get(self.next + ahead) {
            Some(Token::Word(word)) => Some(lower(word)),
            _ => None,
        }
    }

    /// Terms joined by `+` and `-`.
    fn sum(&mut self) -> Result<Value, Failure> {
        let mut value = self.product()?;
        loop {
            let negative = match self.peek() {
                Some(Token::Plus) => false,
                Some(Token::Minus) => true,
                Some(Token::Word(_)) => match self.word(0).as_deref() {
                    Some("plus") => false,
                    Some("minus") => true,
                    _ => break,
                },
                _ => break,
            };
            self.next += 1;
            let right = self.product()?;
            self.operations += 1;
            value = added(value, right, negative)?;
        }
        Ok(value)
    }

    /// Factors joined by `*`, `/`, `mod`, a percentage's `of`, `off` and
    /// `on`, or nothing (`2pi`).
    fn product(&mut self) -> Result<Value, Failure> {
        let mut value = self.signed()?;
        loop {
            let operator = match self.peek() {
                Some(Token::Times) => {
                    self.next += 1;
                    Product::Times
                }
                Some(Token::Divide) => {
                    self.next += 1;
                    Product::Divide
                }
                Some(Token::Open | Token::Root(_)) => Product::Times,
                Some(Token::Number(..))
                    if self.tokens.get(self.next - 1) == Some(&Token::Close) =>
                {
                    Product::Times
                }
                Some(Token::Word(_)) => {
                    let word = self.word(0).unwrap_or_default();
                    let by = self.word(1).as_deref() == Some("by");
                    let (operator, length) = match word.as_str() {
                        "x" | "times" => (Product::Times, 1),
                        "multiplied" if by => (Product::Times, 2),
                        "divided" if by => (Product::Divide, 2),
                        "mod" | "modulo" => (Product::Modulo, 1),
                        "of" if value.percent => (Product::Of, 1),
                        "off" if value.percent => (Product::Off, 1),
                        "on" if value.percent => (Product::On, 1),
                        word if is_constant(word) || is_function(word) => (Product::Times, 0),
                        _ => break,
                    };
                    self.next += length;
                    operator
                }
                _ => break,
            };
            let right = self.signed()?;
            self.operations += 1;
            value = multiplied(value, right, operator)?;
        }
        Ok(value)
    }

    /// A power with any number of leading signs. `-2^2` is `-(2^2)`.
    fn signed(&mut self) -> Result<Value, Failure> {
        let mut negative = false;
        while let Some(sign @ (Token::Minus | Token::Plus)) = self.peek() {
            negative ^= *sign == Token::Minus;
            self.next += 1;
        }
        let mut value = self.power()?;
        if negative {
            value.n = -value.n;
        }
        Ok(value)
    }

    /// An operand, raised to a power if `^` or `power` follows; right to
    /// left.
    fn power(&mut self) -> Result<Value, Failure> {
        let base = self.postfix()?;
        let length = match self.peek() {
            Some(Token::Power) => 1,
            Some(Token::Word(_)) => match self.word(0).as_deref() {
                Some("power") => 1,
                Some("to")
                    if self.word(1).as_deref() == Some("the")
                        && self.word(2).as_deref() == Some("power")
                        && self.word(3).as_deref() == Some("of") =>
                {
                    4
                }
                _ => return Ok(base),
            },
            _ => return Ok(base),
        };
        self.next += length;
        let exponent = self.signed()?;
        self.operations += 1;
        raised(plain(base)?, plain(exponent)?)
    }

    /// An operand and what follows it: `!`, `²`, `³`, `squared`, `cubed`,
    /// `%` or `percent`, a unit, and further quantities of the unit's
    /// dimension (`5 ft 3 in`).
    fn postfix(&mut self) -> Result<Value, Failure> {
        let mut value = self.operand()?;
        loop {
            match self.peek() {
                Some(Token::Bang) => {
                    self.next += 1;
                    self.operations += 1;
                    value = Value::plain(factorial(plain(value)?)?);
                }
                Some(Token::Superscript(power)) => {
                    let power = f64::from(*power);
                    self.next += 1;
                    self.operations += 1;
                    value = raised(plain(value)?, power)?;
                }
                Some(Token::Percent) => {
                    self.next += 1;
                    value = Value {
                        n: plain(value)? / 100.0,
                        unit: None,
                        percent: true,
                    };
                }
                Some(Token::Word(_)) => {
                    let word = self.word(0).unwrap_or_default();
                    match word.as_str() {
                        "percent" if value.is_plain() => {
                            self.next += 1;
                            value.n /= 100.0;
                            value.percent = true;
                        }
                        "squared" | "cubed" => {
                            self.next += 1;
                            self.operations += 1;
                            let power = if word == "squared" { 2.0 } else { 3.0 };
                            value = raised(plain(value)?, power)?;
                        }
                        _ if value.is_plain() => {
                            let Some((unit, length)) = self.unit_at(self.next) else {
                                break;
                            };
                            self.next += length;
                            value.unit = Some(unit);
                            self.more_of(&mut value, unit);
                        }
                        _ => break,
                    }
                }
                _ => break,
            }
        }
        Ok(value)
    }

    /// Adds to `value`, in `unit`, the quantities of the same dimension
    /// written after it without a sign (`5 ft 3 in`, `1 h 30 min`).
    fn more_of(&mut self, value: &mut Value, unit: &'static Unit) {
        while let Some(&Token::Number(n, false)) = self.peek() {
            let Some((next, length)) = self.unit_at(self.next + 1) else {
                break;
            };
            if next.dimension != unit.dimension || next.dimension == Dimension::Temperature {
                break;
            }
            self.next += 1 + length;
            value.n += unit.in_unit(next.to_base(n));
        }
    }

    /// The unit the words from `at` name, and how many tokens name it: two
    /// words (`fl oz`, `light years`), `square` or `cubic` and a length
    /// (`square feet`, `cubic cm`), or one word.
    fn unit_at(&self, at: usize) -> Option<(&'static Unit, usize)> {
        let Some(Token::Word(first)) = self.tokens.get(at) else {
            return None;
        };
        if let Some(Token::Word(second)) = self.tokens.get(at + 1) {
            if units::starts_a_name(first) {
                let mut both = first.clone();
                both.push(' ');
                both.push_str(second);
                if let Some(unit) = units::find(&both) {
                    return Some((unit, 2));
                }
            }
            let power = match lower(first).as_str() {
                "square" | "sq" => Some('²'),
                "cubic" => Some('³'),
                _ => None,
            };
            if let (Some(power), Some(length)) = (power, units::find(second))
                && length.dimension == Dimension::Length
            {
                let mut name = String::from(length.symbol);
                name.push(power);
                if let Some(unit) = units::find(&name) {
                    return Some((unit, 2));
                }
            }
        }
        units::find(first).map(|unit| (unit, 1))
    }

    /// A number, a parenthesized sum, a root, a constant, or a function
    /// and its argument.
    fn operand(&mut self) -> Result<Value, Failure> {
        let Some(token) = self.peek().cloned() else {
            return Err(Failure::Incomplete);
        };
        self.next += 1;
        match token {
            Token::Number(value, prefixed) => {
                self.prefixed |= prefixed;
                Ok(Value::plain(value))
            }
            Token::Open => self.parenthesized(),
            Token::Root(degree) => {
                let value = self.power()?;
                self.operations += 1;
                root(plain(value)?, degree)
            }
            Token::Word(word) => self.named(&lower(&word)),
            _ => Err(Failure::Invalid),
        }
    }

    /// The sum inside a parenthesis already taken, and its closing one.
    fn parenthesized(&mut self) -> Result<Value, Failure> {
        if self.depth == MAX_DEPTH {
            return Err(Failure::Invalid);
        }
        self.depth += 1;
        let value = self.sum()?;
        self.depth -= 1;
        match self.peek() {
            Some(Token::Close) => {
                self.next += 1;
                Ok(value)
            }
            None => Err(Failure::Incomplete),
            Some(_) => Err(Failure::Invalid),
        }
    }

    /// What the word `word`, already taken, begins: a constant, a function,
    /// `square root of`, `cube root of` or `half of`.
    fn named(&mut self, word: &str) -> Result<Value, Failure> {
        if let Some(&(_, value)) = CONSTANTS.iter().find(|(name, _)| *name == word) {
            return Ok(Value::plain(value));
        }
        if matches!(word, "square" | "cube") && self.word(0).as_deref() == Some("root") {
            self.next += 1;
            if self.word(0).as_deref() == Some("of") {
                self.next += 1;
            }
            let value = self.power()?;
            self.operations += 1;
            return root(plain(value)?, if word == "square" { 2 } else { 3 });
        }
        if word == "half" && self.word(0).as_deref() == Some("of") {
            self.next += 1;
            let mut value = self.product()?;
            self.operations += 1;
            value.n /= 2.0;
            return Ok(value);
        }
        if !is_function(word) {
            return Err(Failure::Invalid);
        }
        let argument = if self.peek() == Some(&Token::Open) {
            self.next += 1;
            self.parenthesized()?
        } else {
            self.power()?
        };
        self.operations += 1;
        function(word, argument)
    }
}

fn is_constant(word: &str) -> bool {
    CONSTANTS.iter().any(|(name, _)| *name == word)
}

fn is_function(word: &str) -> bool {
    FUNCTIONS.contains(&word)
}

/// `value`'s number, if it is a plain number.
fn plain(value: Value) -> Result<f64, Failure> {
    if value.is_plain() {
        Ok(value.n)
    } else {
        Err(Failure::Invalid)
    }
}

/// `left` plus (or minus) `right`: a percentage of `left` (`50 + 10%`),
/// two quantities of one dimension in `left`'s unit, or two numbers.
fn added(left: Value, right: Value, negative: bool) -> Result<Value, Failure> {
    let sign = if negative { -1.0 } else { 1.0 };
    if right.percent && !left.percent {
        return Ok(Value {
            n: left.n * (1.0 + sign * right.n),
            ..left
        });
    }
    if left.percent != right.percent {
        return Err(Failure::Invalid);
    }
    let (unit, right_n) = match (left.unit, right.unit) {
        (Some(a), Some(b)) if a.dimension != b.dimension => return Err(Failure::Invalid),
        // A difference, not a reading: 20 °C + 9 °F is 25 °C.
        (Some(a), Some(b)) => (
            Some(a),
            a.in_unit(b.to_base(right.n)) - a.in_unit(b.to_base(0.0)),
        ),
        (Some(a), None) => (Some(a), right.n),
        (None, b) => (b, right.n),
    };
    Ok(Value {
        n: left.n + sign * right_n,
        unit,
        percent: left.percent,
    })
}

/// `left` and `right` combined by `operator`.
fn multiplied(left: Value, right: Value, operator: Product) -> Result<Value, Failure> {
    match operator {
        Product::Of => Ok(Value {
            n: right.n * left.n,
            ..right
        }),
        Product::Off => Ok(Value {
            n: right.n * (1.0 - left.n),
            ..right
        }),
        Product::On => Ok(Value {
            n: right.n * (1.0 + left.n),
            ..right
        }),
        Product::Times => {
            let unit = match (left.unit, right.unit) {
                (Some(_), Some(_)) => return Err(Failure::Invalid),
                (unit, None) | (None, unit) => unit,
            };
            // 50 × 10% is 5; 10% × 2 is 20%; 10% × 20% is 2%.
            let percent = left.percent && (right.percent || right.unit.is_none());
            Ok(Value {
                n: left.n * right.n,
                unit,
                percent,
            })
        }
        Product::Divide | Product::Modulo => {
            let (right_n, unit) = match (left.unit, right.unit) {
                (Some(a), Some(b)) if a.dimension == b.dimension => {
                    let unit = (operator == Product::Modulo).then_some(a);
                    (a.in_unit(b.to_base(right.n)), unit)
                }
                (_, Some(_)) => return Err(Failure::Invalid),
                (unit, None) => (right.n, unit),
            };
            if right_n == 0.0 {
                return Err(Failure::Invalid);
            }
            let n = if operator == Product::Divide {
                left.n / right_n
            } else {
                libm::fmod(left.n, right_n)
            };
            // 10% / 2 is 5%; 50 / 10% is 500.
            let percent = left.percent && !right.percent;
            Ok(Value { n, unit, percent })
        }
    }
}

/// `base` to the power `exponent`; an error where the power is undefined.
fn raised(base: f64, exponent: f64) -> Result<Value, Failure> {
    if base == 0.0 && exponent < 0.0 {
        return Err(Failure::Invalid);
    }
    let value = libm::pow(base, exponent);
    if value.is_finite() {
        Ok(Value::plain(value))
    } else {
        Err(Failure::Invalid)
    }
}

/// `n!` for a whole `n` from 0 to 170.
fn factorial(n: f64) -> Result<f64, Failure> {
    if !(0.0..=170.0).contains(&n) || n != libm::trunc(n) {
        return Err(Failure::Invalid);
    }
    let mut value = 1.0;
    let mut k = 2.0;
    while k <= n {
        value *= k;
        k += 1.0;
    }
    Ok(value)
}

/// The square root (`degree` 2) or cube root (3) of `n`.
fn root(n: f64, degree: u8) -> Result<Value, Failure> {
    match degree {
        2 if n >= 0.0 => Ok(Value::plain(libm::sqrt(n))),
        3 => Ok(Value::plain(libm::cbrt(n))),
        _ => Err(Failure::Invalid),
    }
}

/// The function `name` of `argument`; an error outside its domain.
fn function(name: &str, argument: Value) -> Result<Value, Failure> {
    if matches!(name, "abs" | "round" | "floor" | "ceil") {
        let n = argument.n;
        let n = match name {
            "abs" => n.abs(),
            "round" => libm::round(n),
            "floor" => libm::floor(n),
            _ => libm::ceil(n),
        };
        return Ok(Value { n, ..argument });
    }
    let trigonometric = matches!(name, "sin" | "cos" | "tan");
    let x = match argument.unit {
        Some(unit) if trigonometric && unit.dimension == Dimension::Angle => {
            unit.to_base(argument.n)
        }
        None if !argument.percent => argument.n,
        _ => return Err(Failure::Invalid),
    };
    let n = match name {
        "sqrt" if x >= 0.0 => libm::sqrt(x),
        "cbrt" => libm::cbrt(x),
        "sin" => libm::sin(x),
        "cos" => libm::cos(x),
        "tan" => libm::tan(x),
        "asin" | "arcsin" if (-1.0..=1.0).contains(&x) => libm::asin(x),
        "acos" | "arccos" if (-1.0..=1.0).contains(&x) => libm::acos(x),
        "atan" | "arctan" => libm::atan(x),
        "sinh" => libm::sinh(x),
        "cosh" => libm::cosh(x),
        "tanh" => libm::tanh(x),
        "ln" if x > 0.0 => libm::log(x),
        "log" | "log10" if x > 0.0 => libm::log10(x),
        "log2" if x > 0.0 => libm::log2(x),
        "exp" => libm::exp(x),
        "fact" => factorial(x)?,
        _ => return Err(Failure::Invalid),
    };
    // sin(pi) is 1.2e-16 in doubles: a trigonometric answer that close to
    // a whole number is that number.
    let n = if trigonometric && (n - libm::round(n)).abs() < 1e-12 {
        libm::round(n)
    } else {
        n
    };
    if n.is_finite() {
        Ok(Value::plain(n))
    } else {
        Err(Failure::Invalid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The answer `query` shows, or what it is instead.
    fn shown(query: &str) -> String {
        match evaluate(query) {
            Outcome::Answer(answer) => answer.text,
            Outcome::Incomplete => "incomplete".into(),
            Outcome::Invalid => "invalid".into(),
            Outcome::NotACalculation => "not a calculation".into(),
        }
    }

    /// The text `query`'s answer copies unformatted.
    fn plain_of(query: &str) -> String {
        match evaluate(query) {
            Outcome::Answer(answer) => answer.plain,
            _ => panic!("{query} has no answer"),
        }
    }

    #[test]
    fn arithmetic_keeps_its_precedence() {
        assert_eq!(shown("2 + 2"), "4");
        assert_eq!(shown("2 + 3 * 4"), "14");
        assert_eq!(shown("(2 + 3) * 4"), "20");
        assert_eq!(shown("2 ^ 10"), "1,024");
        assert_eq!(shown("2 ^ 3 ^ 2"), "512");
        assert_eq!(shown("-2^2"), "-4");
        assert_eq!(shown("2 * -(1 + 1)"), "-4");
        assert_eq!(shown("10 - 4 - 3"), "3");
        assert_eq!(shown("1 / 3"), "0.3333333333");
        assert_eq!(shown("0.1 + 0.2"), "0.3");
        assert_eq!(shown(".5 * 4"), "2");
        assert_eq!(shown("2 + 2 ="), "4");
    }

    #[test]
    fn every_multiply_and_divide_sign_is_read() {
        for query in [
            "6 * 7",
            "6 × 7",
            "6 x 7",
            "6x7",
            "6 X 7",
            "6 · 7",
            "6 times 7",
            "6 multiplied by 7",
        ] {
            assert_eq!(shown(query), "42", "{query}");
        }
        for query in ["84 / 2", "84 ÷ 2", "84 divided by 2"] {
            assert_eq!(shown(query), "42", "{query}");
        }
        for query in ["50 - 8", "50 − 8", "50 – 8", "50 minus 8", "34 plus 8"] {
            assert_eq!(shown(query), "42", "{query}");
        }
        assert_eq!(shown("2 ** 5"), "32");
        assert_eq!(shown("2 power 10"), "1,024");
        assert_eq!(shown("3 to the power of 2"), "9");
        assert_eq!(shown("10 mod 3"), "1");
        assert_eq!(shown("-7 mod 3"), "-1");
    }

    #[test]
    fn percentages_read_as_raycasts_do() {
        assert_eq!(shown("20% of 50"), "10");
        assert_eq!(shown("50 + 10%"), "55");
        assert_eq!(shown("50 - 10%"), "45");
        assert_eq!(shown("50 * 10%"), "5");
        assert_eq!(shown("10% off 50"), "45");
        assert_eq!(shown("5% on 30"), "31.5");
        assert_eq!(shown("10 is what % of 50"), "20%");
        assert_eq!(shown("what % of 50 is 10"), "20%");
        assert_eq!(shown("10 as a % of 50"), "20%");
        assert_eq!(shown("10 as % of 50"), "20%");
        assert_eq!(shown("10 is 20% of what"), "50");
        assert_eq!(shown("0.35 as %"), "35%");
        assert_eq!(shown("10% + 5%"), "15%");
        assert_eq!(shown("10% * 2"), "20%");
        assert_eq!(shown("half of 30% of half of 100"), "7.5");
        assert_eq!(shown("20 percent of 50"), "10");
        assert_eq!(shown("10%"), "not a calculation");
        assert_eq!(shown("10% of"), "incomplete");
    }

    #[test]
    fn powers_roots_and_factorials() {
        assert_eq!(shown("5²"), "25");
        assert_eq!(shown("2³"), "8");
        assert_eq!(shown("5 squared"), "25");
        assert_eq!(shown("3 cubed"), "27");
        assert_eq!(shown("√16"), "4");
        assert_eq!(shown("√(9 + 7)"), "4");
        assert_eq!(shown("∛27"), "3");
        assert_eq!(shown("sqrt(16)"), "4");
        assert_eq!(shown("sqrt 625"), "25");
        assert_eq!(shown("cbrt(343)"), "7");
        assert_eq!(shown("square root of 625"), "25");
        assert_eq!(shown("cube root of 27"), "3");
        assert_eq!(shown("5!"), "120");
        assert_eq!(shown("fact(5)"), "120");
        assert_eq!(shown("2 ^ 0.5"), "1.4142135624");
        assert_eq!(shown("2 ^ -1"), "0.5");
        assert_eq!(shown("√-4"), "invalid");
        assert_eq!(shown("171!"), "invalid");
        assert_eq!(shown("0 ^ -1"), "invalid");
    }

    #[test]
    fn constants_and_functions() {
        assert_eq!(shown("2pi"), "6.2831853072");
        assert_eq!(shown("2 * π"), "6.2831853072");
        assert_eq!(shown("e ^ 2"), "7.3890560989");
        assert_eq!(shown("tau / 2"), "3.1415926536");
        assert_eq!(shown("phi * 2"), "3.2360679775");
        assert_eq!(shown("sin(pi / 2)"), "1");
        assert_eq!(shown("sin(pi)"), "0");
        assert_eq!(shown("cos(0)"), "1");
        assert_eq!(shown("sin(90°)"), "1");
        assert_eq!(shown("cos 180 degrees"), "-1");
        assert_eq!(shown("tan(45 deg)"), "1");
        assert_eq!(shown("asin(1) * 2"), "3.1415926536");
        assert_eq!(shown("ln(e)"), "1");
        assert_eq!(shown("log(1000)"), "3");
        assert_eq!(shown("log 100"), "2");
        assert_eq!(shown("log2(8)"), "3");
        assert_eq!(shown("log10(10000)"), "4");
        assert_eq!(shown("exp(0)"), "1");
        assert_eq!(shown("abs(-5)"), "5");
        assert_eq!(shown("round(2.5)"), "3");
        assert_eq!(shown("floor(2.7)"), "2");
        assert_eq!(shown("ceil(2.1)"), "3");
        assert_eq!(shown("ln(0)"), "invalid");
        assert_eq!(shown("asin(2)"), "invalid");
        assert_eq!(shown("pi"), "not a calculation");
    }

    #[test]
    fn multiplication_needs_no_sign_before_a_parenthesis_or_a_name() {
        assert_eq!(shown("3(4 + 5)"), "27");
        assert_eq!(shown("(1 + 2)(3 + 4)"), "21");
        assert_eq!(shown("(2)3"), "6");
        assert_eq!(shown("2 sqrt 9"), "6");
        assert_eq!(shown("2e"), "5.4365636569");
        assert_eq!(shown("2 3"), "invalid");
    }

    #[test]
    fn numbers_take_separators_exponents_and_suffixes() {
        assert_eq!(shown("1,000 + 1"), "1,001");
        assert_eq!(shown("1,234,567 * 2"), "2,469,134");
        assert_eq!(shown("1e3 + 1"), "1,001");
        assert_eq!(shown("1.5E-3 * 2"), "0.003");
        assert_eq!(shown("10k + 5"), "10,005");
        assert_eq!(shown("2M / 4"), "500,000");
        assert_eq!(shown("3bn / 3"), "1,000,000,000");
        assert_eq!(shown("1,00 + 1"), "invalid");
        assert_eq!(shown("1.2.3"), "invalid");
        assert_eq!(shown("42"), "not a calculation");
        assert_eq!(shown("(5)"), "not a calculation");
        assert_eq!(shown("10k"), "not a calculation");
    }

    #[test]
    fn answers_group_their_thousands_but_copy_plain() {
        assert_eq!(shown("4 ^ 6"), "4,096");
        assert_eq!(plain_of("4 ^ 6"), "4096");
        assert_eq!(plain_of("1,500 km in m"), "1500000 m");
    }

    #[test]
    fn number_bases() {
        assert_eq!(shown("0xff"), "255");
        assert_eq!(shown("0xff in decimal"), "255");
        assert_eq!(shown("0b1010 + 1"), "11");
        assert_eq!(shown("0o17"), "15");
        assert_eq!(shown("255 in hex"), "0xFF");
        assert_eq!(shown("256 as hex"), "0x100");
        assert_eq!(shown("99 in binary"), "0b1100011");
        assert_eq!(shown("0b1000101 to octal"), "0o105");
        assert_eq!(shown("0x2D as base 2"), "0b101101");
        assert_eq!(shown("1.5 in hex"), "invalid");
        assert_eq!(shown("10500 in sci"), "1.05e4");
    }

    #[test]
    fn units_convert_with_in_to_and_as() {
        assert_eq!(shown("5 km in miles"), "3.10686 mi");
        assert_eq!(shown("5 km to mi"), "3.10686 mi");
        assert_eq!(shown("10ft in m"), "3.048 m");
        assert_eq!(shown("29 inches to cm"), "73.66 cm");
        assert_eq!(shown("72 in in cm"), "182.88 cm");
        assert_eq!(shown("3 teaspoon in ml"), "14.78676 ml");
        assert_eq!(shown("1 gallon in liters"), "3.78541 l");
        assert_eq!(shown("2 cups in fl oz"), "16 fl oz");
        assert_eq!(shown("1 kg in lb"), "2.20462 lb");
        assert_eq!(shown("16 oz as pounds"), "1 lb");
        assert_eq!(shown("1 acre in m²"), "4,046.85642 m²");
        assert_eq!(shown("1 square mile in square km"), "2.58999 km²");
        assert_eq!(shown("100 sq ft in m2"), "9.2903 m²");
        assert_eq!(shown("90 km/h in mph"), "55.92341 mph");
        assert_eq!(shown("10 m/s to km/h"), "36 km/h");
        assert_eq!(shown("2 hours in minutes"), "120 min");
        assert_eq!(shown("1 h 30 min in min"), "90 min");
        assert_eq!(shown("5 ft 3 in in cm"), "160.02 cm");
        assert_eq!(shown("90 degrees in radians"), "1.5708 rad");
        assert_eq!(shown("2 rem in px"), "32 px");
        assert_eq!(shown("5 km into m"), "5,000 m");
        assert_eq!(shown("5 km in kg"), "invalid");
        assert_eq!(shown("5 in cm"), "invalid");
        assert_eq!(shown("5 km"), "not a calculation");
    }

    #[test]
    fn temperatures_convert_with_their_offsets() {
        assert_eq!(shown("100 f to c"), "37.77778 °C");
        assert_eq!(shown("23C to F"), "73.4 °F");
        assert_eq!(shown("0 °C in K"), "273.15 K");
        assert_eq!(shown("-40 celsius in fahrenheit"), "-40 °F");
        assert_eq!(shown("20 °C + 9 °F"), "25 °C");
    }

    #[test]
    fn data_sizes_are_decimal_or_binary_as_named() {
        assert_eq!(shown("1 GB in MB"), "1,000 MB");
        assert_eq!(shown("1 gb in mb"), "1,000 MB");
        assert_eq!(shown("1 GiB in MiB"), "1,024 MiB");
        assert_eq!(shown("1 GB in GiB"), "0.93132 GiB");
        assert_eq!(shown("100 Mb in MB"), "12.5 MB");
        assert_eq!(shown("1 byte in bits"), "8 bit");
    }

    #[test]
    fn quantities_add_across_their_dimension() {
        assert_eq!(shown("5 km + 300 m"), "5.3 km");
        assert_eq!(shown("2 * 3 km"), "6 km");
        assert_eq!(shown("10 km / 2"), "5 km");
        assert_eq!(shown("1 km / 1 m"), "1,000");
        assert_eq!(shown("50 km + 10%"), "55 km");
        assert_eq!(shown("5 km + 3 kg"), "invalid");
        assert_eq!(shown("2 km * 3 km"), "invalid");
    }

    #[test]
    fn incomplete_and_invalid_queries_have_no_answer() {
        assert_eq!(shown("2 +"), "incomplete");
        assert_eq!(shown("(1 + 2"), "incomplete");
        assert_eq!(shown("sqrt("), "incomplete");
        assert_eq!(shown("hello"), "invalid");
        assert_eq!(shown("2 + * 3"), "invalid");
        assert_eq!(shown("1 / 0"), "invalid");
        assert_eq!(shown("10 mod 0"), "invalid");
        assert_eq!(shown("(1 + 2))"), "invalid");
        assert_eq!(shown("10 ^ 400"), "invalid");
        assert_eq!(shown("1 + 1 = 2"), "invalid");
        assert_eq!(shown(""), "invalid");
        assert_eq!(shown(&"1 + ".repeat(70)), "invalid");
        assert_eq!(shown(&"(".repeat(65)), "invalid");
    }
}
