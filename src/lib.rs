//! Pane's calculator, a default extension: typing a calculation into root
//! search lists its answer first, and invoking the answer copies it (see
//! [`expression`], [`dates`] and [`colour`] for what it understands). Its
//! command is a
//! root provider (`"mode": "provider"` in `pane.json`, #164): it has no row
//! and no screen of its own, only the answer. Disabling the package
//! removes it.
//!
//! The language is plain Rust that does not need Pane's SDK, so its tests
//! run on the computer that builds it (`cargo test`); the SDK and the root
//! provider are built for the component alone, which does without the
//! standard library.
#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

mod colour;
mod dates;
mod expression;
mod format;
mod units;
mod zones;

use alloc::{string::String, vec, vec::Vec};

/// What the calculator makes of a query.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    /// The query's answer.
    Answer(Answer),
    /// The query could become a calculation by typing more, like `2 +` or
    /// `(1 + 2`.
    Incomplete,
    /// The query is not a calculation the calculator knows, or its value
    /// is undefined or too large, like `hello`, `2 + * 3` or `1 / 0`.
    Invalid,
    /// A value without any calculation, like `42` or `5 km`.
    NotACalculation,
}

/// An answer: as it is shown and Enter copies it (`4,096`, `3.10686 mi`,
/// `Dec 25, 2026`), without its thousands grouped (`4096`), the swatch of
/// a colour, and the further copies the Actions panel offers (titled, in
/// order).
#[derive(Debug, PartialEq)]
pub struct Answer {
    pub text: String,
    pub plain: String,
    pub kind: Kind,
    pub swatch: Option<String>,
    pub copies: Vec<(&'static str, String)>,
}

/// What an answer is about, which names the section it is listed under.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// A number, a percentage or a quantity.
    Number,
    /// A date, a time or the time between two.
    DateTime,
    /// A colour.
    Colour,
}

/// When the query was asked: milliseconds since the Unix epoch, and how
/// far local time then is from UTC, in milliseconds.
#[derive(Clone, Copy, Debug)]
pub struct Now {
    pub utc: i64,
    pub offset: i64,
}

/// What the calculator makes of `query`, asked at `now`: a colour first,
/// then a question about dates and times, then an expression.
pub fn calculate(query: &str, now: Now) -> Outcome {
    if let Some(colour) = colour::parse(query) {
        let hex = colour.hex();
        return Outcome::Answer(Answer {
            text: hex.clone(),
            plain: hex.clone(),
            kind: Kind::Colour,
            swatch: Some(hex.clone()),
            copies: vec![
                ("Copy as Hex", hex),
                ("Copy as RGB", colour.rgb()),
                ("Copy as HSL", colour.hsl()),
                ("Copy as OKLCH", colour.oklch()),
            ],
        });
    }
    if let Some(outcome) = dates::evaluate(query, now) {
        return outcome;
    }
    expression::evaluate(query)
}

#[cfg(target_arch = "wasm32")]
mod provider {
    use pane_extension::alloc::{format, vec, vec::Vec};

    use pane_extension::root::{AnswerCopy, AnswerDetail, RootAction, RootResult, WallTime};
    use pane_extension::{Command, NoDesignedView};

    use super::{Kind, Now, Outcome, String, calculate};

    pub struct Calculator;

    /// A root provider: Pane never opens or runs it, so the command keeps
    /// the defaults (opening it is an error).
    impl Command for Calculator {
        type DesignedView = NoDesignedView;
    }

    impl pane_extension::root::Guest for Calculator {
        /// The answer to `query` if it is a calculation with one, else
        /// nothing: ordinary words and incomplete or invalid calculations
        /// are not errors. Enter copies the answer as it is shown, its
        /// thousands grouped, as Raycast's does; the Actions panel also
        /// copies it without them.
        async fn results_for(query: String, at: WallTime) -> Result<Vec<RootResult>, String> {
            let now = Now {
                utc: i64::try_from(at.milliseconds).unwrap_or(i64::MAX),
                offset: at.offset,
            };
            let Outcome::Answer(answer) = calculate(&query, now) else {
                return Ok(Vec::new());
            };
            let section = match answer.kind {
                Kind::Number => "Calculator",
                Kind::DateTime => "Date & Time",
                Kind::Colour => "Color",
            };
            let mut copies: Vec<AnswerCopy> = answer
                .copies
                .into_iter()
                .map(|(title, text)| AnswerCopy {
                    title: title.into(),
                    text,
                })
                .collect();
            if answer.plain != answer.text {
                copies.push(AnswerCopy {
                    title: "Copy Unformatted Answer".into(),
                    text: answer.plain,
                });
            }
            Ok(vec![RootResult {
                id: "answer".into(),
                title: answer.text.clone(),
                subtitle: Some(format!(
                    "{} = {} · Enter copies the answer",
                    query.trim(),
                    answer.text
                )),
                action: RootAction::Copy(answer.text),
                answer: Some(AnswerDetail {
                    section: section.into(),
                    swatch: answer.swatch,
                    copies,
                }),
            }])
        }
    }
}

#[cfg(target_arch = "wasm32")]
use provider::Calculator;

#[cfg(target_arch = "wasm32")]
pane_extension::export!(Calculator);
#[cfg(target_arch = "wasm32")]
pane_extension::root::export!(Calculator);
