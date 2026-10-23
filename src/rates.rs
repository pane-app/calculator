//! The exchange rates currency answers are computed from (pane-app/pane#475):
//! where they come from, how they are kept for offline use, and when they
//! are fetched again.
//!
//! - **Where from.** The primary source is fawazahmed0's currency API on
//!   jsDelivr ([`PRIMARY`]: no key, about 340 currencies with crypto,
//!   updated daily, its data CC0). When it fails — no answer, another
//!   status than 200, a body that is not its rates — the fallback is
//!   ExchangeRate-API's open access endpoint ([`FALLBACK`]: no key, about
//!   165 currencies, updated daily), whose terms ask for attribution
//!   wherever its rates are shown: answers computed from them say "by
//!   Exchange Rate API" ([`Rates::credit`]).
//! - **Kept.** The rates are written to the extension's cache under
//!   [`KEY`] in a small text form of their own ([`Rates::write`]), so the
//!   calculator answers offline once they have been fetched, and reads
//!   them on a query that names a currency without the network.
//! - **When.** A background refresh (the package's continuing service,
//!   `src/lib.rs`) fetches them when there are none or the ones kept were
//!   fetched a day ago or more ([`wait`]), at most once a day, and looks
//!   again every hour, so rates the user cleared with the cache come back
//!   within the hour. A fetch that fails is tried again an hour later.
//!
//! Every rate is how much of a currency one US dollar buys, as both
//! sources give them for `usd`; codes are kept in lower case.

use alloc::{format, string::String, vec::Vec};
use core::future::Future;

/// The primary source's rates for one US dollar.
pub const PRIMARY: &str =
    "https://cdn.jsdelivr.net/npm/@fawazahmed0/currency-api@latest/v1/currencies/usd.json";

/// The fallback source's rates for one US dollar.
pub const FALLBACK: &str = "https://open.er-api.com/v6/latest/USD";

/// The cache key the rates are kept under.
pub const KEY: &str = "rates";

/// A day, in seconds: kept rates this old are fetched again.
pub const DAY: i64 = 86_400;

/// How long the refresh waits, in seconds, before it looks at the kept
/// rates again while they are fresh: an hour.
pub const LOOK: u64 = 3_600;

/// How long the refresh waits, in seconds, after a fetch that failed
/// before it tries again: an hour.
pub const RETRY: u64 = 3_600;

/// The fewest rates an answer of either source must hold to be taken: a
/// body with fewer is not the rates list.
const FEWEST: usize = 30;

/// The first line of the kept form, naming its version.
const HEADER: &str = "pane-calculator-rates 1";

/// Which source the rates came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Primary,
    Fallback,
}

/// The exchange rates: when they were fetched, the day they are for, the
/// source, and each currency's rate for one US dollar.
#[derive(Clone, Debug, PartialEq)]
pub struct Rates {
    /// When they were fetched, in seconds since the Unix epoch.
    pub fetched: i64,
    /// The day they are for, in days since the Unix epoch (UTC).
    pub day: i64,
    pub source: Source,
    /// Lower-case codes and rates, sorted by code.
    per_dollar: Vec<(String, f64)>,
}

impl Rates {
    /// The rates in `per_dollar` (any order, any case), fetched at
    /// `fetched` from `source` for `day`; an error if there are too few
    /// usable ones or none for the US dollar itself.
    fn new(
        fetched: i64,
        day: i64,
        source: Source,
        per_dollar: impl IntoIterator<Item = (String, f64)>,
    ) -> Result<Rates, String> {
        let mut per_dollar: Vec<(String, f64)> = per_dollar
            .into_iter()
            .filter(|(code, rate)| !code.is_empty() && rate.is_finite() && *rate > 0.0)
            .map(|(code, rate)| (code.to_lowercase(), rate))
            .collect();
        per_dollar.sort_by(|a, b| a.0.cmp(&b.0));
        per_dollar.dedup_by(|a, b| a.0 == b.0);
        if per_dollar.len() < FEWEST {
            return Err(format!(
                "the answer holds {} rates, not the rates list",
                per_dollar.len()
            ));
        }
        let rates = Rates {
            fetched,
            day,
            source,
            per_dollar,
        };
        if rates.get("usd").is_none_or(|usd| (usd - 1.0).abs() > 1e-9) {
            return Err("the answer's rates are not for one US dollar".into());
        }
        Ok(rates)
    }

    /// How much of the currency `code` (lower case) one US dollar buys.
    pub fn get(&self, code: &str) -> Option<f64> {
        self.per_dollar
            .binary_search_by(|(known, _)| known.as_str().cmp(code))
            .ok()
            .map(|at| self.per_dollar[at].1)
    }

    /// How many currencies they hold (never none: rates with too few are refused).
    pub fn count(&self) -> usize {
        self.per_dollar.len()
    }

    /// The primary source's answer, `{"date": "2026-10-23", "usd": {"eur":
    /// 0.86, ...}}`, fetched at `fetched`.
    pub fn from_primary(body: &str, fetched: i64) -> Result<Rates, String> {
        let json: serde_json::Value = serde_json::from_str(body)
            .map_err(|error| format!("its answer is not JSON: {error}"))?;
        let day = json
            .get("date")
            .and_then(|date| date.as_str())
            .and_then(iso_day)
            .ok_or("its answer has no date")?;
        let rates = json
            .get("usd")
            .and_then(|rates| rates.as_object())
            .ok_or("its answer has no rates for the US dollar")?;
        Rates::new(
            fetched,
            day,
            Source::Primary,
            rates
                .iter()
                .filter_map(|(code, rate)| Some((code.clone(), rate.as_f64()?))),
        )
    }

    /// The fallback source's answer, `{"result": "success",
    /// "time_last_update_unix": 1792713751, "base_code": "USD", "rates":
    /// {"EUR": 0.86, ...}}`, fetched at `fetched`.
    pub fn from_fallback(body: &str, fetched: i64) -> Result<Rates, String> {
        let json: serde_json::Value = serde_json::from_str(body)
            .map_err(|error| format!("its answer is not JSON: {error}"))?;
        if json.get("result").and_then(|result| result.as_str()) != Some("success") {
            return Err("its answer is not a success".into());
        }
        if json.get("base_code").and_then(|base| base.as_str()) != Some("USD") {
            return Err("its answer is not for the US dollar".into());
        }
        let updated = json
            .get("time_last_update_unix")
            .and_then(|time| time.as_i64())
            .ok_or("its answer has no update time")?;
        let rates = json
            .get("rates")
            .and_then(|rates| rates.as_object())
            .ok_or("its answer has no rates")?;
        Rates::new(
            fetched,
            updated.div_euclid(DAY),
            Source::Fallback,
            rates
                .iter()
                .filter_map(|(code, rate)| Some((code.clone(), rate.as_f64()?))),
        )
    }

    /// The rates in their kept form: a header line, then `fetched`, `day`
    /// and `source` lines, then one `<code> <rate>` line per currency.
    pub fn write(&self) -> String {
        let source = match self.source {
            Source::Primary => "primary",
            Source::Fallback => "fallback",
        };
        let mut text = format!(
            "{HEADER}\nfetched {}\nday {}\nsource {source}\n",
            self.fetched, self.day
        );
        for (code, rate) in &self.per_dollar {
            text.push_str(&format!("{code} {rate}\n"));
        }
        text
    }

    /// Rates in their kept form ([`Rates::write`]); `None` for anything
    /// else, such as a form of another version.
    pub fn read(text: &str) -> Option<Rates> {
        let mut lines = text.lines();
        if lines.next()? != HEADER {
            return None;
        }
        let mut field = |name: &str| {
            lines
                .next()?
                .strip_prefix(name)?
                .strip_prefix(' ')
                .map(String::from)
        };
        let fetched = field("fetched")?.parse().ok()?;
        let day = field("day")?.parse().ok()?;
        let source = match field("source")?.as_str() {
            "primary" => Source::Primary,
            "fallback" => Source::Fallback,
            _ => return None,
        };
        let per_dollar: Option<Vec<(String, f64)>> = lines
            .filter(|line| !line.is_empty())
            .map(|line| {
                let (code, rate) = line.split_once(' ')?;
                Some((code.into(), rate.parse().ok()?))
            })
            .collect();
        Rates::new(fetched, day, source, per_dollar?).ok()
    }

    /// Who the rates must be credited to where they are shown, if anyone:
    /// ExchangeRate-API's terms ask for "Rates By Exchange Rate API".
    pub fn credit(&self) -> Option<&'static str> {
        match self.source {
            Source::Primary => None,
            Source::Fallback => Some("Exchange Rate API"),
        }
    }
}

/// The day `2026-10-23` is, in days since the Unix epoch.
fn iso_day(text: &str) -> Option<i64> {
    let mut parts = text.splitn(3, '-');
    let year: i64 = parts.next()?.parse().ok()?;
    let month: u32 = parts.next()?.parse().ok()?;
    let day: u32 = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(crate::dates::days_from_civil(year, month, day))
}

/// How long the refresh may wait, in seconds, before it looks again, with
/// `kept` the rates kept now (if any) and `now` the time, in seconds since
/// the Unix epoch; `None` when it should fetch now: there are no rates, or
/// they were fetched a day ago or more, or the clock now reads more than a
/// day before they were fetched (it was set back).
pub fn wait(kept: Option<&Rates>, now: i64) -> Option<u64> {
    let kept = kept?;
    let age = now - kept.fetched;
    if !(-DAY..DAY).contains(&age) {
        return None;
    }
    let left = u64::try_from(DAY - age.max(0)).unwrap_or(LOOK);
    Some(left.min(LOOK))
}

/// What a source answered: its status and its body.
pub struct Fetched {
    pub status: u16,
    pub body: String,
}

/// Fetches the rates at `now` with `get` (which sends a GET to an address
/// and reads its whole answer): the primary source's, else the fallback's.
/// An error says why both failed.
pub async fn fetch<F, Fut>(get: F, now: i64) -> Result<Rates, String>
where
    F: Fn(&'static str) -> Fut,
    Fut: Future<Output = Result<Fetched, String>>,
{
    let primary = match answer(&get, PRIMARY).await {
        Ok(body) => match Rates::from_primary(&body, now) {
            Ok(rates) => return Ok(rates),
            Err(error) => error,
        },
        Err(error) => error,
    };
    let fallback = match answer(&get, FALLBACK).await {
        Ok(body) => match Rates::from_fallback(&body, now) {
            Ok(rates) => return Ok(rates),
            Err(error) => error,
        },
        Err(error) => error,
    };
    Err(format!(
        "the currency API failed ({primary}), and so did ExchangeRate-API ({fallback})"
    ))
}

/// The body `url` answered with a 200, else why not.
async fn answer<F, Fut>(get: &F, url: &'static str) -> Result<String, String>
where
    F: Fn(&'static str) -> Fut,
    Fut: Future<Output = Result<Fetched, String>>,
{
    let fetched = get(url).await?;
    if fetched.status != 200 {
        return Err(format!("it answered {}", fetched.status));
    }
    Ok(fetched.body)
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use alloc::string::ToString;
    use core::cell::RefCell;
    use core::pin::pin;
    use core::task::{Context, Poll, Waker};

    /// Runs `future`, which never waits: the fake sources answer at once.
    pub fn ready<T>(future: impl Future<Output = T>) -> T {
        let mut future = pin!(future);
        match future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("a fake source never waits"),
        }
    }

    /// A primary source's answer for 2026-10-23: the dollar, a few real
    /// currencies, crypto, and filler up to the fewest rates taken.
    pub fn primary_body() -> String {
        let mut rates = String::from(
            r#""usd": 1, "eur": 0.86, "gbp": 0.75, "jpy": 152.5, "vnd": 25832, "btc": 0.00001, "eth": 0.00025, "kwd": 0.307"#,
        );
        for n in 0..30 {
            rates.push_str(&format!(r#", "zz{n}": {}"#, n + 1));
        }
        format!(r#"{{"date": "2026-10-23", "usd": {{{rates}}}}}"#)
    }

    /// A fallback source's answer, updated 2026-10-22 00:02:31 UTC, in its
    /// upper-case codes and with no crypto.
    pub fn fallback_body() -> String {
        let mut rates =
            String::from(r#""USD": 1, "EUR": 0.9, "GBP": 0.8, "JPY": 150, "VND": 26000"#);
        for n in 0..30 {
            rates.push_str(&format!(r#", "ZZ{n}": {}"#, n + 1));
        }
        format!(
            r#"{{"result": "success", "time_last_update_unix": 1792627351, "base_code": "USD", "rates": {{{rates}}}}}"#
        )
    }

    /// The primary source's rates, fetched at `fetched`.
    pub fn primary_rates(fetched: i64) -> Rates {
        Rates::from_primary(&primary_body(), fetched).unwrap()
    }

    /// A fake pair of sources: what each address answers, and the
    /// addresses asked, in order.
    struct Fake {
        primary: Result<Fetched, String>,
        fallback: Result<Fetched, String>,
        asked: RefCell<Vec<&'static str>>,
    }

    impl Fake {
        fn new(primary: Result<Fetched, String>, fallback: Result<Fetched, String>) -> Fake {
            Fake {
                primary,
                fallback,
                asked: RefCell::default(),
            }
        }

        fn fetch(&self, now: i64) -> Result<Rates, String> {
            ready(fetch(
                |url| {
                    self.asked.borrow_mut().push(url);
                    let answer = match url {
                        PRIMARY => &self.primary,
                        FALLBACK => &self.fallback,
                        _ => panic!("asked {url}"),
                    };
                    let answer = match answer {
                        Ok(fetched) => Ok(Fetched {
                            status: fetched.status,
                            body: fetched.body.clone(),
                        }),
                        Err(error) => Err(error.clone()),
                    };
                    async move { answer }
                },
                now,
            ))
        }
    }

    fn ok(body: String) -> Result<Fetched, String> {
        Ok(Fetched { status: 200, body })
    }

    const NOW: i64 = 1_792_750_000;

    #[test]
    fn the_primary_source_is_asked_first() {
        let fake = Fake::new(ok(primary_body()), ok(fallback_body()));
        let rates = fake.fetch(NOW).unwrap();
        assert_eq!(*fake.asked.borrow(), [PRIMARY]);
        assert_eq!(rates.source, Source::Primary);
        assert_eq!(rates.fetched, NOW);
        assert_eq!(rates.day, crate::dates::days_from_civil(2026, 10, 23));
        assert_eq!(rates.get("vnd"), Some(25832.0));
        assert_eq!(rates.get("btc"), Some(0.00001));
        assert_eq!(rates.credit(), None);
    }

    #[test]
    fn the_fallback_answers_when_the_primary_fails() {
        let failures = [
            Err("connection refused".to_string()),
            Ok(Fetched {
                status: 503,
                body: primary_body(),
            }),
            ok("<html>not found</html>".into()),
            ok(r#"{"date": "2026-10-23", "usd": {"eur": 0.86}}"#.into()),
        ];
        for primary in failures {
            let fake = Fake::new(primary, ok(fallback_body()));
            let rates = fake.fetch(NOW).unwrap();
            assert_eq!(*fake.asked.borrow(), [PRIMARY, FALLBACK]);
            assert_eq!(rates.source, Source::Fallback);
            assert_eq!(rates.day, crate::dates::days_from_civil(2026, 10, 22));
            assert_eq!(rates.get("eur"), Some(0.9), "codes are kept in lower case");
            assert_eq!(rates.get("btc"), None);
            assert_eq!(rates.credit(), Some("Exchange Rate API"));
        }
    }

    #[test]
    fn both_sources_failing_says_why_each_did() {
        let fake = Fake::new(
            Err("the address could not be resolved".into()),
            Ok(Fetched {
                status: 429,
                body: String::new(),
            }),
        );
        let error = fake.fetch(NOW).unwrap_err();
        assert!(
            error.contains("the address could not be resolved"),
            "{error}"
        );
        assert!(error.contains("it answered 429"), "{error}");
        let fake = Fake::new(
            Err("connecting timed out".into()),
            ok(r#"{"result": "error", "error-type": "unsupported-code"}"#.into()),
        );
        assert!(fake.fetch(NOW).unwrap_err().contains("not a success"));
    }

    #[test]
    fn kept_rates_read_back_as_they_were() {
        for rates in [
            primary_rates(NOW),
            Rates::from_fallback(&fallback_body(), NOW).unwrap(),
        ] {
            assert_eq!(Rates::read(&rates.write()), Some(rates));
        }
        assert_eq!(Rates::read(""), None);
        assert_eq!(Rates::read("pane-calculator-rates 2\nfetched 1\n"), None);
        let mut damaged = primary_rates(NOW).write();
        damaged.push_str("eur many\n");
        assert_eq!(Rates::read(&damaged), None);
    }

    #[test]
    fn rates_are_fetched_at_most_once_a_day() {
        let kept = primary_rates(NOW);
        assert_eq!(wait(None, NOW), None, "no rates: fetch now");
        assert_eq!(
            wait(Some(&kept), NOW),
            Some(LOOK),
            "fresh: look again in an hour"
        );
        assert_eq!(wait(Some(&kept), NOW + DAY - 600), Some(600));
        assert_eq!(wait(Some(&kept), NOW + DAY - 1), Some(1));
        assert_eq!(wait(Some(&kept), NOW + DAY), None, "a day old: fetch again");
        assert_eq!(wait(Some(&kept), NOW + 3 * DAY), None);
        assert_eq!(
            wait(Some(&kept), NOW - 60),
            Some(LOOK),
            "the clock moved back a little"
        );
        assert_eq!(
            wait(Some(&kept), NOW - DAY - 1),
            None,
            "the clock was set back"
        );
    }
}
