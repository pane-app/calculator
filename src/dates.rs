//! Questions about dates and times, after what Raycast's calculator reads
//! (pane-app/pane#472), answered from the moment of the query and its
//! local offset from UTC, offline:
//!
//! - `now`, `today`, `tomorrow`, `yesterday`, `next monday`, `last friday`,
//!   `tomorrow at 5pm`;
//! - a date or time and amounts of time added or taken away: `now + 3
//!   days`, `today - 1 month`, `christmas + 1 week`, `in 3 hours`,
//!   `3 days from now`, `35 days ago`, `2 weeks after 1 may`,
//!   `monday in 3 weeks` (the Monday of the week three weeks from today);
//! - the time between two: `days until christmas`, `days until 25 dec`,
//!   `weeks until 2027-01-01`, `hours until 5pm`, `days since 1 jan`,
//!   `days between 3 march and 30 may`, `how many days until halloween`;
//! - the time in a place (see [`crate::zones`]): `time in Tokyo`,
//!   `Tokyo time`, `what time is it in London`, and a time of day from one
//!   place, or here, in another: `5pm London in New York`, `9am in sf`;
//! - Unix timestamps: `1700000000 to date` (seconds, or milliseconds from
//!   10^11 up), `now in unix`, `christmas to timestamp`, `unix time`.
//!
//! Dates are written as `25 dec`, `dec 25`, `25th december 2026`,
//! `2026-12-25`, a weekday, or `christmas`, `christmas eve`, `new year`,
//! `new year's eve`, `valentine's day`, `halloween` and `easter`; times as
//! `5pm`, `5:30 pm`, `17:30`, `noon` and `midnight`. A date without a year
//! is its next one, or its last one after `since`. Times are written on
//! the 24-hour clock (`17:05 Fri 23 Oct`), with the year only when it is
//! not this one; dates always with their year (`Fri 25 Dec 2026`).
//!
//! Local time is the query's offset from UTC throughout: an amount of time
//! that crosses a change to or from daylight saving keeps the offset of
//! the moment asked at.

use alloc::{format, string::String, vec::Vec};

use crate::zones::{self, Zone};
use crate::{Answer, Kind, Now, Outcome};

/// A day in milliseconds.
pub const DAY: i64 = 86_400_000;
const HOUR: i64 = 3_600_000;
const MINUTE: i64 = 60_000;

/// The day `year`-`month`-`day` (proleptic Gregorian) in days since the
/// Unix epoch (Howard Hinnant's `days_from_civil`).
pub fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let of_era = year - era * 400;
    let month = i64::from(month);
    let of_year =
        (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + i64::from(day) - 1;
    let of_era_days = of_era * 365 + of_era / 4 - of_era / 100 + of_year;
    era * 146_097 + of_era_days - 719_468
}

/// The year, month and day of `days` since the Unix epoch.
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let of_era = days - era * 146_097;
    let year_of_era = (of_era - of_era / 1460 + of_era / 36_524 - of_era / 146_096) / 365;
    let of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted = (5 * of_year + 2) / 153;
    let day = (of_year - (153 * shifted + 2) / 5 + 1) as u32;
    let month = if shifted < 10 {
        shifted + 3
    } else {
        shifted - 9
    } as u32;
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// The weekday of `days` since the Unix epoch: 0 for Sunday to 6 for
/// Saturday.
pub fn weekday(days: i64) -> u32 {
    (days + 4).rem_euclid(7) as u32
}

fn days_in_month(year: i64, month: u32) -> u32 {
    let next = if month == 12 {
        days_from_civil(year + 1, 1, 1)
    } else {
        days_from_civil(year, month + 1, 1)
    };
    (next - days_from_civil(year, month, 1)) as u32
}

const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// The weekday a word names, 0 for Sunday.
fn weekday_named(word: &str) -> Option<u32> {
    Some(match word {
        "sunday" | "sun" => 0,
        "monday" | "mon" => 1,
        "tuesday" | "tue" | "tues" => 2,
        "wednesday" | "wed" => 3,
        "thursday" | "thu" | "thur" | "thurs" => 4,
        "friday" | "fri" => 5,
        "saturday" | "sat" => 6,
        _ => return None,
    })
}

/// The month a word names, 1 for January.
fn month_named(word: &str) -> Option<u32> {
    Some(match word {
        "january" | "jan" => 1,
        "february" | "feb" => 2,
        "march" | "mar" => 3,
        "april" | "apr" => 4,
        "may" => 5,
        "june" | "jun" => 6,
        "july" | "jul" => 7,
        "august" | "aug" => 8,
        "september" | "sep" | "sept" => 9,
        "october" | "oct" => 10,
        "november" | "nov" => 11,
        "december" | "dec" => 12,
        _ => return None,
    })
}

/// An amount of time a word names.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Span {
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Year,
}

fn span_named(word: &str) -> Option<(Span, i64)> {
    Some(match word {
        "second" | "seconds" | "sec" | "secs" => (Span::Second, 1),
        "minute" | "minutes" | "min" | "mins" => (Span::Minute, 1),
        "hour" | "hours" | "hr" | "hrs" => (Span::Hour, 1),
        "day" | "days" => (Span::Day, 1),
        "week" | "weeks" | "wk" | "wks" => (Span::Week, 1),
        "fortnight" | "fortnights" => (Span::Week, 2),
        "month" | "months" => (Span::Month, 1),
        "year" | "years" | "yr" | "yrs" => (Span::Year, 1),
        _ => return None,
    })
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Word(String),
    Number(i64),
    /// A time of day, in milliseconds since midnight.
    Clock(i64),
    /// A date written as `2026-12-25`, in days since the Unix epoch.
    Date(i64),
    Plus,
    Minus,
}

/// A moment in local time: milliseconds since the Unix epoch as if local
/// time were UTC, and whether it has a time of day or is a whole day.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Moment {
    local: i64,
    time: bool,
}

impl Moment {
    fn day(days: i64) -> Moment {
        Moment {
            local: days * DAY,
            time: false,
        }
    }

    fn days(&self) -> i64 {
        self.local.div_euclid(DAY)
    }

    /// This moment `count` spans later (earlier for a negative count).
    fn plus(self, count: i64, span: Span) -> Option<Moment> {
        let milliseconds = |unit: i64| {
            Some(Moment {
                local: self.local.checked_add(count.checked_mul(unit)?)?,
                time: self.time || unit < DAY,
            })
        };
        match span {
            Span::Second => milliseconds(1000),
            Span::Minute => milliseconds(MINUTE),
            Span::Hour => milliseconds(HOUR),
            Span::Day => milliseconds(DAY),
            Span::Week => milliseconds(7 * DAY),
            Span::Month | Span::Year => {
                let months = if span == Span::Year {
                    count.checked_mul(12)?
                } else {
                    count
                };
                // Ten thousand years either way, so that the day's
                // milliseconds stay well inside an i64.
                if months.unsigned_abs() > 120_000 {
                    return None;
                }
                let (year, month, day) = civil_from_days(self.days());
                let index = year.checked_mul(12)? + i64::from(month) - 1 + months;
                let (year, month) = (index.div_euclid(12), (index.rem_euclid(12) + 1) as u32);
                let day = day.min(days_in_month(year, month));
                let of_day = self.local.rem_euclid(DAY);
                Some(Moment {
                    local: days_from_civil(year, month, day) * DAY + of_day,
                    time: self.time,
                })
            }
        }
    }
}

/// Which occurrence a date without a year is: the next one or the last.
#[derive(Clone, Copy, PartialEq)]
enum Toward {
    Future,
    Past,
}

/// Where a time of day is: here, or in a place.
#[derive(Clone, Copy)]
enum Place {
    Here,
    Zone(Zone),
}

/// What `query`, asked at `now`, asks about dates or times; `None` when it
/// is not such a question, for the expression language to read.
pub fn evaluate(query: &str, now: Now) -> Option<Outcome> {
    let tokens = tokens(query)?;
    // A date alone is no calculation, and 2026-12-25 no subtraction.
    if let [Token::Date(_)] = tokens.as_slice() {
        return Some(Outcome::NotACalculation);
    }
    let parser = Parser {
        tokens: &tokens,
        next: 0,
        now,
    };
    let forms: [fn(&mut Parser<'_>) -> Option<Reply>; 7] = [
        |parser| parser.between(),
        |parser| parser.time_in(),
        |parser| parser.clock_in(),
        |parser| parser.unix(),
        |parser| parser.relative(),
        |parser| parser.weekday_in(),
        |parser| parser.moment_alone(),
    ];
    for form in forms {
        let mut parser = Parser { next: 0, ..parser };
        if let Some(reply) = form(&mut parser)
            && parser.next == tokens.len()
        {
            return Some(match reply {
                Reply::Answer(text, plain) => Outcome::Answer(Answer {
                    text,
                    plain,
                    kind: Kind::DateTime,
                }),
                Reply::NotACalculation => Outcome::NotACalculation,
            });
        }
    }
    // A date written as 2026-12-25 among other words is still no
    // subtraction.
    if tokens.iter().any(|token| matches!(token, Token::Date(_))) {
        return Some(Outcome::Invalid);
    }
    None
}

/// What a form makes of the query.
enum Reply {
    /// The answer, as shown and plain.
    Answer(String, String),
    NotACalculation,
}

impl Reply {
    fn same(text: String) -> Reply {
        Reply::Answer(text.clone(), text)
    }
}

/// `query` as the words, numbers, times and dates it is made of; `None`
/// if it has anything else.
fn tokens(query: &str) -> Option<Vec<Token>> {
    let query = query
        .trim()
        .to_lowercase()
        .replace(['\u{2212}', '\u{2013}'], "-")
        .replace(['\u{2019}', '\''], "")
        .replace('+', " + ");
    let mut tokens = Vec::new();
    for chunk in query.split_whitespace() {
        let mut chunk = chunk.trim_end_matches(',');
        loop {
            if let Some(rest) = chunk.strip_prefix('+') {
                tokens.push(Token::Plus);
                chunk = rest;
            } else if let Some(rest) = chunk.strip_prefix('-')
                && (rest.is_empty() || rest.starts_with(|c: char| c.is_ascii_digit()))
            {
                tokens.push(Token::Minus);
                chunk = rest;
            } else {
                break;
            }
        }
        if chunk.is_empty() {
            continue;
        }
        tokens.push(token(chunk)?);
    }
    (!tokens.is_empty()).then_some(tokens)
}

fn token(chunk: &str) -> Option<Token> {
    if let Some(date) = iso_date(chunk) {
        return Some(Token::Date(date));
    }
    if let Some(clock) = clock(chunk) {
        return Some(Token::Clock(clock));
    }
    let digits = chunk.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    let suffix = &chunk[digits.len()..];
    if !digits.is_empty()
        && digits.len() <= 15
        && digits.bytes().all(|b| b.is_ascii_digit())
        && matches!(suffix, "" | "st" | "nd" | "rd" | "th")
    {
        return digits.parse().ok().map(Token::Number);
    }
    if chunk.chars().all(|c| c.is_alphabetic() || c == '.') {
        return Some(Token::Word(chunk.into()));
    }
    None
}

/// `2026-12-25` in days since the Unix epoch.
fn iso_date(chunk: &str) -> Option<i64> {
    let mut parts = chunk.split('-');
    let (year, month, day) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || year.len() != 4 {
        return None;
    }
    let year: i64 = year.parse().ok()?;
    let month: u32 = month.parse().ok()?;
    let day: u32 = day.parse().ok()?;
    if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
        return None;
    }
    Some(days_from_civil(year, month, day))
}

/// `5pm`, `5:30pm`, `12am` or `17:30` in milliseconds since midnight.
fn clock(chunk: &str) -> Option<i64> {
    let (digits, half) =
        if let Some(digits) = chunk.strip_suffix("am").or(chunk.strip_suffix("a.m.")) {
            (digits, Some(false))
        } else if let Some(digits) = chunk.strip_suffix("pm").or(chunk.strip_suffix("p.m.")) {
            (digits, Some(true))
        } else {
            (chunk, None)
        };
    let (hour, minute) = match digits.split_once(':') {
        Some((hour, minute)) if minute.len() == 2 => (hour, minute),
        Some(_) => return None,
        // `17` alone is a number, not a time.
        None if half.is_some() => (digits, "00"),
        None => return None,
    };
    if hour.is_empty() || hour.len() > 2 || !hour.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let hour: i64 = hour.parse().ok()?;
    let minute: i64 = minute.parse().ok()?;
    if minute >= 60 {
        return None;
    }
    let hour = match half {
        Some(pm) if (1..=12).contains(&hour) => hour % 12 + if pm { 12 } else { 0 },
        Some(_) => return None,
        None if hour < 24 => hour,
        None => return None,
    };
    Some(hour * HOUR + minute * MINUTE)
}

#[derive(Clone, Copy)]
struct Parser<'a> {
    tokens: &'a [Token],
    next: usize,
    now: Now,
}

impl Parser<'_> {
    fn word(&self, ahead: usize) -> Option<&str> {
        match self.tokens.get(self.next + ahead) {
            Some(Token::Word(word)) => Some(word),
            _ => None,
        }
    }

    /// Takes the word `text` if it is next.
    fn take(&mut self, text: &str) -> bool {
        let found = self.word(0) == Some(text);
        if found {
            self.next += 1;
        }
        found
    }

    fn local_now(&self) -> i64 {
        self.now.utc + self.now.offset
    }

    fn today(&self) -> i64 {
        self.local_now().div_euclid(DAY)
    }

    fn this_year(&self) -> i64 {
        civil_from_days(self.today()).0
    }

    /// `days until M`, `days since M`, `days between M and N`, `days from
    /// M to N`, and the same in other spans, after an optional `how
    /// many`.
    fn between(&mut self) -> Option<Reply> {
        if self.word(0) == Some("how") && self.word(1) == Some("many") {
            self.next += 2;
        }
        let (span, _) = span_named(self.word(0)?)?;
        self.next += 1;
        self.take("left");
        let now = Moment {
            local: self.local_now(),
            time: true,
        };
        let (from, to) = match self.word(0)? {
            "until" | "till" | "til" | "to" | "before" => {
                self.next += 1;
                (now, self.moment_sum(Toward::Future)?)
            }
            "since" | "after" => {
                self.next += 1;
                (self.moment_sum(Toward::Past)?, now)
            }
            "between" | "from" => {
                let between = self.word(0) == Some("between");
                self.next += 1;
                let from = self.moment_sum(Toward::Future)?;
                let joined = if between {
                    self.take("and")
                } else {
                    self.take("to") || self.take("until") || self.take("till")
                };
                if !joined {
                    return None;
                }
                (from, self.moment_sum(Toward::Future)?)
            }
            _ => return None,
        };
        Some(Reply::Answer(
            span_between(from, to, span, true),
            span_between(from, to, span, false),
        ))
    }

    /// `time in P`, `now in P`, `time at P`, `what time is it in P`,
    /// `P time`, `time P`.
    fn time_in(&mut self) -> Option<Reply> {
        if self.word(0) == Some("what")
            && self.word(1) == Some("time")
            && self.word(2) == Some("is")
        {
            self.next += 3;
            self.take("it");
            if !(self.take("in") || self.take("at")) {
                return None;
            }
        } else if self.take("time") || self.take("now") {
            let _ = self.take("in") || self.take("at");
        } else {
            let end = self.tokens.len().checked_sub(1)?;
            if !matches!(&self.tokens[end], Token::Word(word) if word == "time") {
                return None;
            }
            let zone = self.place_until(end)?;
            self.next = self.tokens.len();
            return Some(self.zone_time(zone));
        }
        let zone = self.place_until(self.tokens.len())?;
        self.next = self.tokens.len();
        Some(self.zone_time(zone))
    }

    /// The time now in `zone`.
    fn zone_time(&self, zone: Zone) -> Reply {
        let local = self.now.utc + zone.offset(self.now.utc);
        Reply::same(self.written(Moment { local, time: true }))
    }

    /// The place the words from `next` up to `end` name.
    fn place_until(&self, end: usize) -> Option<Zone> {
        let words: Option<Vec<&str>> = self.tokens[self.next..end]
            .iter()
            .map(|token| match token {
                Token::Word(word) => Some(word.as_str()),
                _ => None,
            })
            .collect();
        let words = words?;
        if words.is_empty() {
            return None;
        }
        zones::find(&words.join(" "))
    }

    /// `5pm P in Q`, `5pm in Q`, `17:30 P to Q`: a time of day today in
    /// one place (here without one), in another.
    fn clock_in(&mut self) -> Option<Reply> {
        let clock = self.clock()?;
        let split = (self.next..self.tokens.len()).find(
            |&at| matches!(&self.tokens[at], Token::Word(word) if word == "in" || word == "to"),
        )?;
        let from = if split == self.next {
            Place::Here
        } else {
            Place::Zone(self.place_until(split)?)
        };
        self.next = split + 1;
        let to = self.place_until(self.tokens.len())?;
        self.next = self.tokens.len();
        let offset = |place: Place, utc: i64| match place {
            Place::Here => self.now.offset,
            Place::Zone(zone) => zone.offset(utc),
        };
        // Today in the place the time is in, at that time; its offset is
        // looked up again at that moment, which is near enough to settle.
        let day = (self.now.utc + offset(from, self.now.utc)).div_euclid(DAY);
        let local = day * DAY + clock;
        let guess = local - offset(from, self.now.utc);
        let utc = local - offset(from, guess);
        let there = utc + to.offset(utc);
        Some(Reply::same(self.written(Moment {
            local: there,
            time: true,
        })))
    }

    /// A time of day: `5pm`, `17:30`, `5 pm`, `noon`, `midnight`.
    fn clock(&mut self) -> Option<i64> {
        match self.tokens.get(self.next)? {
            Token::Clock(clock) => {
                self.next += 1;
                Some(*clock)
            }
            Token::Number(hour) if (1..=12).contains(hour) => {
                let pm = match self.word(1)? {
                    "am" | "a.m." => false,
                    "pm" | "p.m." => true,
                    _ => return None,
                };
                self.next += 2;
                Some((hour % 12 + if pm { 12 } else { 0 }) * HOUR)
            }
            Token::Word(word) if word == "noon" || word == "midday" => {
                self.next += 1;
                Some(12 * HOUR)
            }
            Token::Word(word) if word == "midnight" => {
                self.next += 1;
                Some(0)
            }
            _ => None,
        }
    }

    /// `N to date` (seconds, or milliseconds from 10^11), `M to unix`,
    /// `M as timestamp`, and `unix time`, `unix timestamp`, `current
    /// timestamp`, `epoch time` alone for now.
    fn unix(&mut self) -> Option<Reply> {
        let now_alone = matches!(
            (self.word(0), self.word(1)),
            (Some("unix"), Some("time" | "timestamp"))
                | (Some("current"), Some("timestamp" | "unix"))
                | (Some("epoch"), Some("time"))
        );
        if now_alone {
            self.next += 2;
            if self.word(0) == Some("time") || self.word(0) == Some("timestamp") {
                self.next += 1;
            }
            return Some(Reply::same(format!("{}", self.now.utc.div_euclid(1000))));
        }
        if let (Some(Token::Number(n)), Some("to" | "in" | "as")) =
            (self.tokens.get(self.next), self.word(1))
            && matches!(self.word(2), Some("date" | "time" | "datetime"))
        {
            let n = *n;
            self.next += 3;
            let utc = if n >= 100_000_000_000 {
                n
            } else {
                n.checked_mul(1000)?
            };
            return Some(Reply::same(self.written(Moment {
                local: utc.checked_add(self.now.offset)?,
                time: true,
            })));
        }
        let split = (self.next..self.tokens.len())
            .rev()
            .find(|&at| matches!(&self.tokens[at], Token::Word(word) if matches!(word.as_str(), "to" | "in" | "as")))?;
        let target: Vec<&Token> = self.tokens[split + 1..].iter().collect();
        let unix = matches!(
            target.as_slice(),
            [Token::Word(a)] if matches!(a.as_str(), "unix" | "timestamp" | "epoch")
        ) || matches!(
            target.as_slice(),
            [Token::Word(a), Token::Word(b)] if a == "unix" && (b == "time" || b == "timestamp")
        );
        if !unix {
            return None;
        }
        let mut before = Parser {
            tokens: &self.tokens[..split],
            ..*self
        };
        let moment = before.moment_sum(Toward::Future)?;
        if before.next != split {
            return None;
        }
        self.next = self.tokens.len();
        let utc = moment.local - self.now.offset;
        Some(Reply::same(format!("{}", utc.div_euclid(1000))))
    }

    /// `in N spans`, `N spans ago`, `N spans from now`, `N spans later`,
    /// `N spans after M`, `N spans before M`.
    fn relative(&mut self) -> Option<Reply> {
        let ahead = self.take("in");
        let (count, span) = self.amount()?;
        if ahead {
            return Some(Reply::same(
                self.written(self.counted_from(span).plus(count, span)?),
            ));
        }
        let moment = match self.word(0)? {
            "ago" => {
                self.next += 1;
                self.counted_from(span).plus(-count, span)?
            }
            "later" | "hence" => {
                self.next += 1;
                self.counted_from(span).plus(count, span)?
            }
            "from" if matches!(self.word(1), Some("now" | "today")) => {
                self.next += 2;
                self.counted_from(span).plus(count, span)?
            }
            "after" | "from" => {
                self.next += 1;
                self.moment_sum(Toward::Future)?.plus(count, span)?
            }
            "before" => {
                self.next += 1;
                self.moment_sum(Toward::Future)?.plus(-count, span)?
            }
            _ => return None,
        };
        Some(Reply::same(self.written(moment)))
    }

    /// What an amount of `span` counts from: now for hours and shorter,
    /// today for days and longer.
    fn counted_from(&self, span: Span) -> Moment {
        match span {
            Span::Second | Span::Minute | Span::Hour => Moment {
                local: self.local_now(),
                time: true,
            },
            _ => Moment::day(self.today()),
        }
    }

    /// `monday in 3 weeks`: that weekday in the week (Monday to Sunday)
    /// three weeks from today.
    fn weekday_in(&mut self) -> Option<Reply> {
        let wanted = weekday_named(self.word(0)?)?;
        self.next += 1;
        if !self.take("in") {
            return None;
        }
        let (count, span) = self.amount()?;
        if span != Span::Week {
            return None;
        }
        let later = self.today() + 7 * count;
        let monday = later - i64::from((weekday(later) + 6) % 7);
        let day = monday + i64::from((wanted + 6) % 7);
        Some(Reply::same(self.written(Moment::day(day))))
    }

    /// A moment and the amounts added to or taken from it: `now + 3 days`.
    /// A moment alone answers only when it is relative to today (`today`,
    /// `tomorrow`, `next monday`); a date alone (`25 dec`) is no
    /// calculation.
    fn moment_alone(&mut self) -> Option<Reply> {
        let start = self.next;
        let moment = self.moment(Toward::Future)?;
        let relative = self.relative_word(start);
        let mut sum = *self;
        let moment = sum.amounts(moment)?;
        let changed = sum.next != self.next;
        *self = sum;
        if changed || relative {
            Some(Reply::same(self.written(moment)))
        } else {
            Some(Reply::NotACalculation)
        }
    }

    /// Whether the moment read from `start` is relative to now.
    fn relative_word(&self, start: usize) -> bool {
        matches!(
            self.tokens.get(start),
            Some(Token::Word(word)) if matches!(
                word.as_str(),
                "now" | "today" | "tomorrow" | "yesterday" | "next" | "last" | "this"
            )
        )
    }

    /// A moment, then the amounts added to or taken from it.
    fn moment_sum(&mut self, toward: Toward) -> Option<Moment> {
        let moment = self.moment(toward)?;
        self.amounts(moment)
    }

    /// `moment` with each `+ N spans` or `- N spans` (or `plus`, `minus`)
    /// that follows.
    fn amounts(&mut self, mut moment: Moment) -> Option<Moment> {
        loop {
            let negative = match self.tokens.get(self.next) {
                Some(Token::Plus) => false,
                Some(Token::Minus) => true,
                Some(Token::Word(word)) if word == "plus" => false,
                Some(Token::Word(word)) if word == "minus" => true,
                _ => return Some(moment),
            };
            self.next += 1;
            let (count, span) = self.amount()?;
            moment = moment.plus(if negative { -count } else { count }, span)?;
        }
    }

    /// `3 days`, `a week`, `an hour`, `2 fortnights`.
    fn amount(&mut self) -> Option<(i64, Span)> {
        let count = match self.tokens.get(self.next)? {
            Token::Number(n) => *n,
            Token::Word(word) if matches!(word.as_str(), "a" | "an" | "one") => 1,
            _ => return None,
        };
        let (span, times) = span_named(self.word(1)?)?;
        self.next += 2;
        Some((count.checked_mul(times)?, span))
    }

    /// A moment: `now`, `today`, `tomorrow`, `yesterday`, `next`, `last`
    /// or `this` and a weekday, `week`, `month` or `year`, a weekday, a
    /// holiday, a date, or a time of day today; a day may be followed by
    /// a time of day (`tomorrow at 5pm`).
    fn moment(&mut self, toward: Toward) -> Option<Moment> {
        let today = self.today();
        let day = match self.tokens.get(self.next)? {
            Token::Word(word) => match word.as_str() {
                "now" => {
                    self.next += 1;
                    return Some(Moment {
                        local: self.local_now(),
                        time: true,
                    });
                }
                "noon" | "midday" | "midnight" => {
                    let clock = self.clock()?;
                    return Some(self.at_clock(clock, toward));
                }
                "today" => {
                    self.next += 1;
                    today
                }
                "tomorrow" => {
                    self.next += 1;
                    today + 1
                }
                "yesterday" => {
                    self.next += 1;
                    today - 1
                }
                "next" | "last" | "this" => {
                    let which = word.clone();
                    let after = self.word(1)?;
                    let forward = which != "last";
                    if let Some(wanted) = weekday_named(after) {
                        self.next += 2;
                        let gap = (i64::from(wanted) - i64::from(weekday(today))).rem_euclid(7);
                        match which.as_str() {
                            "next" => today + if gap == 0 { 7 } else { gap },
                            "this" => today + gap,
                            _ => today - (7 - gap) % 7 - if gap == 0 { 7 } else { 0 },
                        }
                    } else if let Some((span, _)) = span_named(after)
                        && matches!(span, Span::Week | Span::Month | Span::Year)
                    {
                        self.next += 2;
                        Moment::day(today)
                            .plus(if forward { 1 } else { -1 }, span)?
                            .days()
                    } else {
                        return None;
                    }
                }
                _ => {
                    if let Some(wanted) = weekday_named(word) {
                        self.next += 1;
                        let gap = (i64::from(wanted) - i64::from(weekday(today))).rem_euclid(7);
                        match toward {
                            Toward::Future => today + gap,
                            Toward::Past => today - (7 - gap) % 7,
                        }
                    } else if let Some(month) = month_named(word) {
                        // dec 25, dec 25 2026
                        let Some(Token::Number(day)) = self.tokens.get(self.next + 1) else {
                            return None;
                        };
                        let day = u32::try_from(*day).ok()?;
                        self.next += 2;
                        self.dated(month, day, toward)?
                    } else {
                        self.holiday(toward)?
                    }
                }
            },
            Token::Number(day) => {
                // 25 dec, 25 december 2026; else 5 pm
                let month = self.word(1).and_then(month_named);
                match month {
                    Some(month) => {
                        let day = u32::try_from(*day).ok()?;
                        self.next += 2;
                        self.dated(month, day, toward)?
                    }
                    None => {
                        let clock = self.clock()?;
                        return Some(self.at_clock(clock, toward));
                    }
                }
            }
            Token::Date(day) => {
                self.next += 1;
                *day
            }
            Token::Clock(_) => {
                let clock = self.clock()?;
                return Some(self.at_clock(clock, toward));
            }
            _ => return None,
        };
        // tomorrow at 5pm, 25 dec 9:00
        let mut at = *self;
        at.take("at");
        if let Some(clock) = at.clock() {
            *self = at;
            return Some(Moment {
                local: day * DAY + clock,
                time: true,
            });
        }
        Some(Moment::day(day))
    }

    /// A time of day `clock` today, or the next (or last) time it is.
    fn at_clock(&self, clock: i64, toward: Toward) -> Moment {
        let today = self.today() * DAY + clock;
        let local = match toward {
            Toward::Future if today < self.local_now() => today + DAY,
            Toward::Past if today > self.local_now() => today - DAY,
            _ => today,
        };
        Moment { local, time: true }
    }

    /// The `day` of `month` in the year that follows, or its next (or
    /// last) occurrence without one.
    fn dated(&mut self, month: u32, day: u32, toward: Toward) -> Option<i64> {
        if let Some(Token::Number(year)) = self.tokens.get(self.next)
            && (1000..=9999).contains(year)
        {
            let year = *year;
            self.next += 1;
            return valid(year, month, day);
        }
        self.occurrence(toward, |year| valid(year, month, day))
    }

    /// A holiday's next (or last) day: `christmas`, `christmas eve`,
    /// `new year`, `new years eve`, `valentines day`, `halloween`, `easter`,
    /// and the year after it, if one follows.
    fn holiday(&mut self, toward: Toward) -> Option<i64> {
        let word = self.word(0)?;
        let second = self.word(1);
        let (date, length): (fn(i64) -> Option<i64>, usize) = match (word, second) {
            ("christmas" | "xmas", Some("eve")) => (|year| valid(year, 12, 24), 2),
            ("christmas" | "xmas", Some("day")) => (|year| valid(year, 12, 25), 2),
            ("christmas" | "xmas", _) => (|year| valid(year, 12, 25), 1),
            ("new", Some("year" | "years")) => match self.word(2) {
                Some("eve") => (|year| valid(year, 12, 31), 3),
                Some("day") => (|year| valid(year, 1, 1), 3),
                _ => (|year| valid(year, 1, 1), 2),
            },
            ("halloween", _) => (|year| valid(year, 10, 31), 1),
            ("valentines" | "valentine", Some("day")) => (|year| valid(year, 2, 14), 2),
            ("valentines" | "valentine", _) => (|year| valid(year, 2, 14), 1),
            ("easter", Some("sunday")) => (|year| Some(easter(year)), 2),
            ("easter", _) => (|year| Some(easter(year)), 1),
            _ => return None,
        };
        self.next += length;
        if let Some(Token::Number(year)) = self.tokens.get(self.next)
            && (1000..=9999).contains(year)
        {
            let year = *year;
            self.next += 1;
            return date(year);
        }
        self.occurrence(toward, date)
    }

    /// The day `date` gives for some year that is the next on or after
    /// today, or the last on or before it.
    fn occurrence(&self, toward: Toward, date: impl Fn(i64) -> Option<i64>) -> Option<i64> {
        let today = self.today();
        let year = self.this_year();
        let this = date(year)?;
        Some(match toward {
            Toward::Future if this < today => date(year + 1)?,
            Toward::Past if this > today => date(year - 1)?,
            _ => this,
        })
    }

    /// `moment` as the calculator writes it.
    fn written(&self, moment: Moment) -> String {
        let days = moment.days();
        let (year, month, day) = civil_from_days(days);
        let date = format!(
            "{} {day} {}",
            WEEKDAYS[weekday(days) as usize],
            MONTHS[month as usize - 1]
        );
        if !moment.time {
            return format!("{date} {year}");
        }
        let of_day = moment.local.rem_euclid(DAY);
        let clock = format!("{:02}:{:02}", of_day / HOUR, of_day % HOUR / MINUTE);
        if year == self.this_year() {
            format!("{clock} {date}")
        } else {
            format!("{clock} {date} {year}")
        }
    }
}

/// The day `year`-`month`-`day`, if there is one.
fn valid(year: i64, month: u32, day: u32) -> Option<i64> {
    (day >= 1 && day <= days_in_month(year, month)).then(|| days_from_civil(year, month, day))
}

/// Easter Sunday of `year` (the anonymous Gregorian computus).
fn easter(year: i64) -> i64 {
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = (h + l - 7 * m + 114) % 31 + 1;
    days_from_civil(year, month as u32, day as u32)
}

/// The time from `from` to `to` in `span`s, as the calculator writes it:
/// whole days, weeks and days, months and days, years and months, hours
/// and minutes, minutes or seconds; `… ago` when `to` is before `from`.
fn span_between(from: Moment, to: Moment, span: Span, grouped: bool) -> String {
    let (from, to, past) = if to.local < from.local {
        (to, from, true)
    } else {
        (from, to, false)
    };
    let count = |n: i64, one: &str, many: &str| {
        let number = crate::format::plain(n as f64, grouped);
        if n == 1 {
            format!("1 {one}")
        } else {
            format!("{number} {many}")
        }
    };
    let days = to.days() - from.days();
    let text = match span {
        Span::Day => count(days, "day", "days"),
        Span::Week => {
            let weeks = count(days / 7, "week", "weeks");
            match days % 7 {
                0 => weeks,
                rest => format!("{weeks}, {}", count(rest, "day", "days")),
            }
        }
        Span::Month | Span::Year => {
            let (y1, m1, d1) = civil_from_days(from.days());
            let (y2, m2, d2) = civil_from_days(to.days());
            let mut months = (y2 - y1) * 12 + i64::from(m2) - i64::from(m1);
            if d2 < d1 {
                months -= 1;
            }
            let after = Moment::day(from.days())
                .plus(months, Span::Month)
                .map_or(0, |moment| moment.days());
            let rest = to.days() - after;
            if span == Span::Year {
                let years = count(months / 12, "year", "years");
                match months % 12 {
                    0 => years,
                    rest => format!("{years}, {}", count(rest, "month", "months")),
                }
            } else {
                let whole = count(months, "month", "months");
                match rest {
                    0 => whole,
                    rest => format!("{whole}, {}", count(rest, "day", "days")),
                }
            }
        }
        Span::Hour => {
            let minutes = (to.local - from.local) / MINUTE;
            let hours = count(minutes / 60, "hour", "hours");
            match minutes % 60 {
                0 => hours,
                rest => format!("{hours}, {}", count(rest, "minute", "minutes")),
            }
        }
        Span::Minute => count((to.local - from.local) / MINUTE, "minute", "minutes"),
        Span::Second => count((to.local - from.local) / 1000, "second", "seconds"),
    };
    if past { format!("{text} ago") } else { text }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Friday 23 October 2026 at 16:05 in Hanoi (UTC+7).
    fn now() -> Now {
        Now {
            utc: days_from_civil(2026, 10, 23) * DAY + 9 * HOUR + 5 * MINUTE,
            offset: 7 * HOUR,
        }
    }

    fn shown_at(query: &str, now: Now) -> String {
        match evaluate(query, now) {
            Some(Outcome::Answer(answer)) => answer.text,
            Some(Outcome::NotACalculation) => "not a calculation".into(),
            Some(other) => panic!("{query}: {other:?}"),
            None => "not a date".into(),
        }
    }

    fn shown(query: &str) -> String {
        shown_at(query, now())
    }

    #[test]
    fn the_calendar_arithmetic_is_right() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(days_from_civil(2024, 2, 29)), (2024, 2, 29));
        assert_eq!(civil_from_days(days_from_civil(1600, 3, 1)), (1600, 3, 1));
        assert_eq!(weekday(days_from_civil(2026, 10, 23)), 5);
        assert_eq!(civil_from_days(easter(2026)), (2026, 4, 5));
        assert_eq!(civil_from_days(easter(2027)), (2027, 3, 28));
    }

    #[test]
    fn today_and_now() {
        assert_eq!(shown("now"), "16:05 Fri 23 Oct");
        assert_eq!(shown("today"), "Fri 23 Oct 2026");
        assert_eq!(shown("Tomorrow"), "Sat 24 Oct 2026");
        assert_eq!(shown("yesterday"), "Thu 22 Oct 2026");
        assert_eq!(shown("next monday"), "Mon 26 Oct 2026");
        assert_eq!(shown("next friday"), "Fri 30 Oct 2026");
        assert_eq!(shown("last friday"), "Fri 16 Oct 2026");
        assert_eq!(shown("last monday"), "Mon 19 Oct 2026");
        assert_eq!(shown("next week"), "Fri 30 Oct 2026");
        assert_eq!(shown("tomorrow at 5pm"), "17:00 Sat 24 Oct");
        assert_eq!(shown("tomorrow 9:30"), "09:30 Sat 24 Oct");
    }

    #[test]
    fn amounts_of_time_move_a_moment() {
        assert_eq!(shown("now + 3 days"), "16:05 Mon 26 Oct");
        assert_eq!(shown("now+3 days"), "16:05 Mon 26 Oct");
        assert_eq!(shown("today + 2 weeks"), "Fri 6 Nov 2026");
        assert_eq!(shown("today - 1 month"), "Wed 23 Sep 2026");
        assert_eq!(shown("now plus 2 hours"), "18:05 Fri 23 Oct");
        assert_eq!(shown("christmas + 1 week"), "Fri 1 Jan 2027");
        assert_eq!(shown("31 jan + 1 month"), "Sun 28 Feb 2027");
        assert_eq!(shown("in 3 days"), "Mon 26 Oct 2026");
        assert_eq!(shown("in 3 hours"), "19:05 Fri 23 Oct");
        assert_eq!(shown("3 days from now"), "Mon 26 Oct 2026");
        assert_eq!(shown("35 days ago"), "Fri 18 Sep 2026");
        assert_eq!(shown("a week ago"), "Fri 16 Oct 2026");
        assert_eq!(shown("2 weeks after 1 may"), "Sat 15 May 2027");
        assert_eq!(shown("monday in 3 weeks"), "Mon 9 Nov 2026");
        assert_eq!(shown("1 year ago"), "Thu 23 Oct 2025");
    }

    #[test]
    fn the_time_between_two_moments() {
        assert_eq!(shown("days until christmas"), "63 days");
        assert_eq!(shown("days until Christmas"), "63 days");
        assert_eq!(shown("how many days until christmas"), "63 days");
        assert_eq!(shown("days until 25 dec"), "63 days");
        assert_eq!(shown("days until dec 25"), "63 days");
        assert_eq!(shown("days until 25th december"), "63 days");
        assert_eq!(shown("days until 2026-12-25"), "63 days");
        assert_eq!(shown("weeks until christmas"), "9 weeks");
        assert_eq!(shown("weeks until new year"), "10 weeks");
        assert_eq!(shown("days until halloween"), "8 days");
        assert_eq!(shown("days until tomorrow"), "1 day");
        assert_eq!(shown("days since 1 jan"), "295 days");
        assert_eq!(shown("days since christmas"), "302 days");
        assert_eq!(shown("days between 3 march and 30 may"), "88 days");
        assert_eq!(shown("days from 1 jan 2026 to 1 jan 2027"), "365 days");
        assert_eq!(shown("months until christmas"), "2 months, 2 days");
        assert_eq!(shown("years since 2000-01-01"), "26 years, 9 months");
        assert_eq!(shown("hours until midnight"), "7 hours, 55 minutes");
        assert_eq!(shown("hours until tomorrow"), "7 hours, 55 minutes");
        assert_eq!(shown("minutes until 5pm"), "55 minutes");
        assert_eq!(shown("hours until 9am"), "16 hours, 55 minutes");
        assert_eq!(shown("minutes since 4pm"), "5 minutes");
        assert_eq!(shown("days until 2026-01-01"), "295 days ago");
        assert_eq!(shown("days until easter"), "156 days");
    }

    #[test]
    fn the_time_in_a_place() {
        assert_eq!(shown("time in tokyo"), "18:05 Fri 23 Oct");
        assert_eq!(shown("time in Tokyo"), "18:05 Fri 23 Oct");
        assert_eq!(shown("tokyo time"), "18:05 Fri 23 Oct");
        assert_eq!(shown("what time is it in london"), "10:05 Fri 23 Oct");
        assert_eq!(shown("time in new york"), "05:05 Fri 23 Oct");
        assert_eq!(shown("now in sydney"), "20:05 Fri 23 Oct");
        assert_eq!(shown("time in kathmandu"), "14:50 Fri 23 Oct");
        assert_eq!(shown("time in honolulu"), "23:05 Thu 22 Oct");
        assert_eq!(shown("time in atlantis"), "not a date");
    }

    #[test]
    fn a_time_of_day_in_another_place() {
        assert_eq!(shown("5pm london in new york"), "12:00 Fri 23 Oct");
        assert_eq!(shown("5pm ldn in sf"), "09:00 Fri 23 Oct");
        assert_eq!(shown("5 pm in tokyo"), "19:00 Fri 23 Oct");
        assert_eq!(shown("9am sf to london"), "17:00 Fri 23 Oct");
        assert_eq!(shown("17:30 tokyo in paris"), "10:30 Fri 23 Oct");
        assert_eq!(shown("noon in new york"), "01:00 Fri 23 Oct");
    }

    #[test]
    fn unix_timestamps() {
        assert_eq!(shown("1700000000 to date"), "05:13 Wed 15 Nov 2023");
        assert_eq!(shown("1700000000000 to date"), "05:13 Wed 15 Nov 2023");
        assert_eq!(shown("now in unix"), "1792746300");
        assert_eq!(shown("unix time"), "1792746300");
        assert_eq!(shown("current timestamp"), "1792746300");
        assert_eq!(shown("2026-10-24 to timestamp"), "1792774800");
    }

    #[test]
    fn a_date_alone_is_no_calculation() {
        assert_eq!(shown("2026-12-25"), "not a calculation");
        assert_eq!(shown("25 dec"), "not a calculation");
        assert_eq!(shown("christmas"), "not a calculation");
        assert_eq!(shown("friday"), "not a calculation");
    }

    #[test]
    fn amounts_too_large_have_no_answer() {
        for query in [
            "now + 999999999999999 years",
            "999999999999999 days ago",
            "today + 999999999 months",
        ] {
            assert!(
                !matches!(evaluate(query, now()), Some(Outcome::Answer(_))),
                "{query}"
            );
        }
    }

    #[test]
    fn other_queries_are_left_to_the_expression() {
        for query in [
            "2 + 2",
            "5 days in hours",
            "5 km in miles",
            "hello",
            "2 hours",
            "17",
            "time",
            "2026-12-25 + 1",
        ] {
            assert!(
                !matches!(evaluate(query, now()), Some(Outcome::Answer(_))),
                "{query}"
            );
        }
    }

    #[test]
    fn daylight_saving_is_kept_in_the_place_asked_about() {
        // 15 January 2026, 12:00 UTC.
        let winter = Now {
            utc: days_from_civil(2026, 1, 15) * DAY + 12 * HOUR,
            offset: 0,
        };
        assert_eq!(shown_at("time in new york", winter), "07:00 Thu 15 Jan");
        assert_eq!(shown_at("time in london", winter), "12:00 Thu 15 Jan");
        assert_eq!(shown_at("time in sydney", winter), "23:00 Thu 15 Jan");
    }
}
