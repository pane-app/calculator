//! Pane's calculator, a default extension: typing a calculation into root
//! search lists its answer first, and invoking the answer copies it (see
//! [`expression`], [`dates`] and [`colour`] for what it understands). Its
//! command is a
//! root provider (`"mode": "provider"` in `pane.json`, #164): it has no row
//! and no screen of its own, only the answer. Disabling the package
//! removes it.
//!
//! It also converts currencies ([`currency`], pane-app/pane#475) with
//! exchange rates it fetches from the web and keeps in its cache for
//! offline use ([`rates`]). Answering never waits for the network: an
//! answer reads the rates kept, and a second command, **Refresh Currency
//! Rates** (`refresh-rates`, `"mode": "no-view"`), fetches them — in the
//! background as the package's continuing service (`"service": true`),
//! when there are none and then at most once a day, and at once when the
//! user runs it. `pane.json` names the same component twice, at
//! `dist/calculator.wasm` for the calculator and `dist/rates/calculator.wasm`
//! for the refresh: Pane runs one call at a time in an instance of a
//! component file, so with a file of its own a fetch waiting on the
//! network never holds the calculator's answers behind it.
//!
//! The language is plain Rust that does not need Pane's SDK, so its tests
//! run on the computer that builds it (`cargo test`); the SDK and the root
//! provider are built for the component alone, which does without the
//! standard library.
#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

mod colour;
pub mod currency;
mod dates;
mod expression;
mod format;
pub mod rates;
mod units;
mod zones;

use alloc::{string::String, vec, vec::Vec};

use rates::Rates;

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
    /// An amount converted into another currency; its section names the
    /// day of its rates ([`Money::Answer`]).
    Currency,
}

/// What the calculator answers about currencies.
#[derive(Debug, PartialEq)]
pub enum Money {
    /// The conversion, the section it is listed under ("Currency · rates
    /// from 23 Oct") and what it says of its rates ("rates from 23 Oct").
    Answer {
        answer: Answer,
        section: String,
        note: String,
    },
    /// The query converts currencies, but no rates have been fetched yet.
    NotLoaded,
}

/// What the calculator answers about currencies for `query`, asked on
/// `today` (days since the Unix epoch, local): `None` when the query
/// converts no currency, or one the rates do not hold, so the rest of the
/// calculator answers it. `kept` reads the rates kept and `home` the user's
/// currency (lower case); neither is called for a query that names no
/// currency, so typing anything else reads nothing.
pub fn money(
    query: &str,
    kept: impl FnOnce() -> Option<Rates>,
    home: impl FnOnce() -> String,
    today: i64,
) -> Option<Money> {
    let request = currency::request(query)?;
    let Some(rates) = kept() else {
        return request.is_known().then_some(Money::NotLoaded);
    };
    let answer = currency::convert(&request, &rates, &home())?;
    Some(Money::Answer {
        answer,
        section: currency::section(&rates, today),
        note: currency::note(&rates, today),
    })
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
    use pane_extension::alloc::{format, string::ToString, vec, vec::Vec};

    use pane_extension::LaunchRecord;
    use pane_extension::feedback::{Toast, show_toast};
    use pane_extension::root::{AnswerCopy, AnswerDetail, RootAction, RootResult, WallTime};
    use pane_extension::service::Cycle;
    use pane_extension::{Command, NoDesignedView, cache};

    use super::rates::{self, Fetched, Rates};
    use super::{Answer, Kind, Money, Now, Outcome, String, calculate, currency, money};

    /// The calculator's command, which answers root search.
    const CALCULATOR: &str = "calculator";

    /// The command that fetches the rates: its run and its service.
    const REFRESH: &str = "refresh-rates";

    pub struct Calculator;

    /// The calculator is a root provider: Pane never opens or runs it.
    /// Refresh Currency Rates is a no-view command, run when the user
    /// chooses it.
    impl Command for Calculator {
        type DesignedView = NoDesignedView;

        /// Fetches the rates now, whenever they were last fetched: the user
        /// asked. The toast says how it went.
        async fn run(command: String, _launch: LaunchRecord) -> Result<(), String> {
            if command != REFRESH {
                return Err(format!("`{command}` cannot be run"));
            }
            let shown = show_toast(Toast::animated("Refreshing currency rates…"));
            match refresh(now()).await {
                Ok(rates) => shown.update(
                    Toast::success("Currency rates refreshed").message(described(&rates, now())),
                ),
                Err(error) => shown
                    .update(Toast::failure("Could not refresh the currency rates").message(error)),
            }
            Ok(())
        }
    }

    /// The refresh in the background: rates when there are none, then at
    /// most once a day (see [`rates::wait`]). A failed fetch is no error of
    /// the service's, which Pane would run again a second later: the cycle
    /// says why, and tries again in an hour.
    impl pane_extension::service::Guest for Calculator {
        async fn run_cycle(_command: String) -> Result<Cycle, String> {
            let at = now();
            let kept = kept();
            if let Some(wait) = rates::wait(kept.as_ref(), at) {
                let status = kept
                    .as_ref()
                    .map_or_else(String::new, |rates| described(rates, at));
                return Ok(Cycle {
                    status,
                    next_seconds: wait,
                });
            }
            Ok(match refresh(at).await {
                Ok(rates) => Cycle {
                    status: described(&rates, at),
                    next_seconds: rates::LOOK,
                },
                Err(error) => Cycle {
                    status: format!("Could not fetch currency rates: {error}"),
                    next_seconds: rates::RETRY,
                },
            })
        }
    }

    /// Now, in seconds since the Unix epoch, by the system's clock.
    fn now() -> i64 {
        wasip3::clocks::system_clock::now().seconds
    }

    /// The rates kept in the cache, if any.
    fn kept() -> Option<Rates> {
        cache::get(rates::KEY)
            .ok()
            .flatten()
            .and_then(|text| Rates::read(&text))
    }

    /// Fetches the rates at `at` and keeps them.
    async fn refresh(at: i64) -> Result<Rates, String> {
        let rates = rates::fetch(
            |url| async move {
                let response =
                    pane_extension::http::get(url, &[("accept", "application/json")]).await?;
                Ok(Fetched {
                    status: response.status,
                    body: response.text(),
                })
            },
            at,
        )
        .await?;
        cache::set(rates::KEY, &rates.write())?;
        Ok(rates)
    }

    /// `rates` in words, at `at`: "Rates from 23 Oct, 341 currencies".
    fn described(rates: &Rates, at: i64) -> String {
        let note = currency::note(rates, at.div_euclid(rates::DAY));
        let mut text = note[..1].to_uppercase();
        text.push_str(&note[1..]);
        format!("{text}, {} currencies", rates.count())
    }

    /// The user's currency, as the "Your currency" preference names it.
    fn home() -> String {
        let preference = pane_extension::preferences::json(Some(CALCULATOR))
            .ok()
            .and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok())
            .and_then(|values| Some(values.get("currency")?.as_str()?.to_string()));
        currency::home(preference.as_deref())
    }

    /// The card of `answer` to `query`, under `section`, its subtitle
    /// saying `note` too when it has one.
    fn card(query: &str, answer: Answer, section: &str, note: Option<&str>) -> RootResult {
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
        let note = note.map(|note| format!(" · {note}")).unwrap_or_default();
        RootResult {
            id: "answer".into(),
            title: answer.text.clone(),
            subtitle: Some(format!(
                "{} = {}{note} · Enter copies the answer",
                query.trim(),
                answer.text
            )),
            action: RootAction::Copy(answer.text),
            answer: Some(AnswerDetail {
                section: section.into(),
                swatch: answer.swatch,
                copies,
            }),
        }
    }

    impl pane_extension::root::Guest for Calculator {
        /// The answer to `query` if it is a calculation with one, else
        /// nothing: ordinary words and incomplete or invalid calculations
        /// are not errors. Enter copies the answer as it is shown, its
        /// thousands grouped, as Raycast's does; the Actions panel also
        /// copies it without them. A currency conversion is answered from
        /// the rates kept, never waiting for the network; before any have
        /// been fetched, its one result says so.
        async fn results_for(query: String, at: WallTime) -> Result<Vec<RootResult>, String> {
            let now = Now {
                utc: i64::try_from(at.milliseconds).unwrap_or(i64::MAX),
                offset: at.offset,
            };
            let today = now.utc.saturating_add(now.offset).div_euclid(86_400_000);
            match money(&query, kept, home, today) {
                Some(Money::Answer {
                    answer,
                    section,
                    note,
                }) => return Ok(vec![card(&query, answer, &section, Some(&note))]),
                Some(Money::NotLoaded) => {
                    return Ok(vec![RootResult {
                        id: "rates-not-loaded".into(),
                        title: "Currency rates are not loaded yet".into(),
                        subtitle: Some(
                            "They are fetched in the background once Pane is online, then kept \
                             for offline use; Refresh Currency Rates fetches them now"
                                .into(),
                        ),
                        action: RootAction::Copy(query.trim().into()),
                        answer: None,
                    }]);
                }
                None => {}
            }
            let Outcome::Answer(answer) = calculate(&query, now) else {
                return Ok(Vec::new());
            };
            let section = match answer.kind {
                Kind::Number | Kind::Currency => "Calculator",
                Kind::DateTime => "Date & Time",
                Kind::Colour => "Color",
            };
            Ok(vec![card(&query, answer, section, None)])
        }
    }
}

#[cfg(target_arch = "wasm32")]
use provider::Calculator;

#[cfg(target_arch = "wasm32")]
pane_extension::export!(Calculator);
#[cfg(target_arch = "wasm32")]
pane_extension::root::export!(Calculator);
#[cfg(target_arch = "wasm32")]
pane_extension::service::export!(Calculator);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rates::tests::primary_rates;

    const TODAY: i64 = 20_750;

    #[test]
    fn a_conversion_is_answered_from_the_rates_kept() {
        let Some(Money::Answer {
            answer,
            section,
            note,
        }) = money(
            "100 usd in vnd",
            || Some(primary_rates(0)),
            || "usd".into(),
            TODAY,
        )
        else {
            panic!("no answer");
        };
        assert_eq!(answer.text, "2,583,200 VND");
        assert_eq!(answer.plain, "2583200");
        assert_eq!(section, "Currency · rates from 23 Oct");
        assert_eq!(note, "rates from 23 Oct");
    }

    #[test]
    fn offline_with_no_rates_the_answer_says_they_are_not_loaded() {
        let none = || None;
        assert_eq!(
            money("100 usd in vnd", none, || "usd".into(), TODAY),
            Some(Money::NotLoaded)
        );
        assert_eq!(
            money("€50", none, || "usd".into(), TODAY),
            Some(Money::NotLoaded)
        );
        // A code only the rates could hold is no currency without them.
        assert_eq!(money("5 cubed", none, || "usd".into(), TODAY), None);
        assert!(matches!(
            calculate("5 cubed", Now { utc: 0, offset: 0 }),
            Outcome::Answer(_)
        ));
    }

    #[test]
    fn queries_that_name_no_currency_read_nothing() {
        let unread = || -> Option<Rates> { panic!("the rates were read") };
        let no_home = || -> String { panic!("the preference was read") };
        for query in [
            "2 + 2",
            "5 km in miles",
            "time in Tokyo",
            "#ff8800",
            "hello",
        ] {
            assert_eq!(money(query, unread, no_home, TODAY), None, "{query}");
        }
    }

    #[test]
    fn a_currency_the_rates_lack_is_left_to_the_calculator() {
        let kept = || Some(primary_rates(0));
        assert_eq!(money("5 cubed", kept, || "usd".into(), TODAY), None);
        assert_eq!(money("1 dogs in usd", kept, || "usd".into(), TODAY), None);
    }
}
