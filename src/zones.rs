//! The places the calculator knows the time in (`time in Tokyo`,
//! `5pm London in New York`), offline: each place's standard offset from
//! UTC and the daylight-saving rule it follows now — the United States'
//! and Canada's, the European Union's and the United Kingdom's, south-east
//! Australia's and New Zealand's. Places whose daylight saving follows
//! another rule (Chile, Israel, Egypt, Paraguay…) are left out rather than
//! answered wrongly for part of the year.

use crate::dates::{DAY, civil_from_days, days_from_civil, weekday};

const HOUR: i64 = 3_600_000;

/// When a place keeps daylight saving time, an hour ahead of its standard
/// time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Rule {
    Never,
    /// From the second Sunday of March at 2:00 to the first Sunday of
    /// November at 2:00, local time.
    UnitedStates,
    /// From the last Sunday of March to the last Sunday of October, at
    /// 1:00 UTC.
    Europe,
    /// From the first Sunday of October at 2:00 to the first Sunday of
    /// April at 3:00, local time.
    Australia,
    /// From the last Sunday of September at 2:00 to the first Sunday of
    /// April at 3:00, local time.
    NewZealand,
}

/// A place's time: its standard offset from UTC and its daylight saving.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zone {
    /// Minutes ahead of UTC in standard time.
    pub standard: i64,
    pub rule: Rule,
}

use Rule::*;

/// The places, by their names in lower case, their standard offsets in
/// minutes and their rules.
#[rustfmt::skip]
static PLACES: &[(&[&str], i64, Rule)] = &[
    (&["utc", "gmt", "zulu"], 0, Never),
    (&["london", "ldn", "uk", "united kingdom", "england", "britain", "great britain", "scotland", "edinburgh", "manchester", "bst"], 0, Europe),
    (&["dublin", "ireland", "lisbon", "portugal"], 0, Europe),
    (&["reykjavik", "iceland", "accra", "dakar"], 0, Never),
    (&["paris", "france", "berlin", "germany", "munich", "frankfurt", "hamburg", "madrid", "spain", "barcelona", "rome", "italy", "milan", "amsterdam", "netherlands", "brussels", "belgium", "vienna", "austria", "zurich", "geneva", "switzerland", "stockholm", "sweden", "oslo", "norway", "copenhagen", "denmark", "prague", "czechia", "warsaw", "poland", "budapest", "hungary", "belgrade", "serbia", "zagreb", "croatia", "luxembourg", "cet", "cest"], 60, Europe),
    (&["lagos", "nigeria", "algiers", "tunis"], 60, Never),
    (&["athens", "greece", "helsinki", "finland", "kyiv", "kiev", "ukraine", "bucharest", "romania", "sofia", "bulgaria", "riga", "latvia", "tallinn", "estonia", "vilnius", "lithuania", "eet", "eest"], 120, Europe),
    (&["johannesburg", "cape town", "south africa", "harare", "kigali"], 120, Never),
    (&["istanbul", "turkey", "ankara", "moscow", "russia", "msk", "minsk", "riyadh", "saudi arabia", "doha", "qatar", "kuwait", "baghdad", "nairobi", "kenya", "addis ababa"], 180, Never),
    (&["tehran", "iran"], 210, Never),
    (&["dubai", "abu dhabi", "uae", "united arab emirates", "muscat", "oman", "baku"], 240, Never),
    (&["kabul"], 270, Never),
    (&["karachi", "pakistan", "islamabad", "lahore", "tashkent"], 300, Never),
    (&["delhi", "new delhi", "mumbai", "bombay", "bangalore", "bengaluru", "kolkata", "calcutta", "chennai", "hyderabad", "india", "ist", "colombo", "sri lanka"], 330, Never),
    (&["kathmandu", "nepal"], 345, Never),
    (&["dhaka", "bangladesh", "almaty"], 360, Never),
    (&["yangon", "myanmar"], 390, Never),
    (&["bangkok", "thailand", "hanoi", "ho chi minh city", "ho chi minh", "saigon", "vietnam", "jakarta", "indonesia", "phnom penh", "cambodia"], 420, Never),
    (&["singapore", "kuala lumpur", "malaysia", "hong kong", "hk", "beijing", "shanghai", "shenzhen", "guangzhou", "china", "taipei", "taiwan", "manila", "philippines", "perth", "bali"], 480, Never),
    (&["tokyo", "osaka", "kyoto", "japan", "jst", "seoul", "south korea", "korea", "kst"], 540, Never),
    (&["darwin"], 570, Never),
    (&["adelaide"], 570, Australia),
    (&["brisbane", "queensland"], 600, Never),
    (&["sydney", "melbourne", "canberra", "hobart", "aest", "aedt"], 600, Australia),
    (&["auckland", "wellington", "new zealand", "nz"], 720, NewZealand),
    (&["honolulu", "hawaii", "hst"], -600, Never),
    (&["anchorage", "alaska"], -540, UnitedStates),
    (&["los angeles", "la", "san francisco", "sf", "seattle", "portland", "san diego", "las vegas", "vancouver", "silicon valley", "pst", "pdt"], -480, UnitedStates),
    (&["denver", "salt lake city", "calgary", "edmonton", "mst", "mdt"], -420, UnitedStates),
    (&["phoenix", "arizona"], -420, Never),
    (&["chicago", "dallas", "houston", "austin", "minneapolis", "new orleans", "winnipeg", "cst", "cdt"], -360, UnitedStates),
    (&["mexico city", "mexico", "guatemala", "costa rica"], -360, Never),
    (&["new york", "nyc", "ny", "boston", "washington", "washington dc", "dc", "miami", "atlanta", "philadelphia", "detroit", "toronto", "montreal", "ottawa", "est", "edt"], -300, UnitedStates),
    (&["bogota", "bogotá", "colombia", "lima", "peru", "panama", "quito"], -300, Never),
    (&["halifax"], -240, UnitedStates),
    (&["caracas", "venezuela", "la paz", "santo domingo", "puerto rico"], -240, Never),
    (&["st johns", "st. john's", "st john's", "newfoundland"], -210, UnitedStates),
    (&["sao paulo", "são paulo", "rio de janeiro", "rio", "brasilia", "brasília", "brazil", "buenos aires", "argentina", "montevideo", "uruguay"], -180, Never),
];

/// The zone of the place `name` names, compared without case.
pub fn find(name: &str) -> Option<Zone> {
    let name = name.trim().to_lowercase();
    PLACES
        .iter()
        .find(|(names, _, _)| names.contains(&name.as_str()))
        .map(|&(_, standard, rule)| Zone { standard, rule })
}

impl Zone {
    /// How far the place's time is ahead of UTC at `utc` (milliseconds
    /// since the Unix epoch), in milliseconds.
    pub fn offset(&self, utc: i64) -> i64 {
        let standard = self.standard * 60_000;
        if self.daylight_saving(utc, standard) {
            standard + HOUR
        } else {
            standard
        }
    }

    fn daylight_saving(&self, utc: i64, standard: i64) -> bool {
        let (year, _, _) = civil_from_days((utc + standard).div_euclid(DAY));
        // An hour of standard time on a day, as UTC.
        let at = |day: i64, hour: i64| day * DAY + hour * HOUR - standard;
        match self.rule {
            Never => false,
            UnitedStates => {
                // 2:00 in daylight time is 1:00 in standard time.
                let start = at(nth_sunday(year, 3, 2), 2);
                let end = at(nth_sunday(year, 11, 1), 1);
                (start..end).contains(&utc)
            }
            Europe => {
                let start = last_sunday(year, 3) * DAY + HOUR;
                let end = last_sunday(year, 10) * DAY + HOUR;
                (start..end).contains(&utc)
            }
            Australia => {
                let end = at(nth_sunday(year, 4, 1), 2);
                let start = at(nth_sunday(year, 10, 1), 2);
                utc < end || utc >= start
            }
            NewZealand => {
                let end = at(nth_sunday(year, 4, 1), 2);
                let start = at(last_sunday(year, 9), 2);
                utc < end || utc >= start
            }
        }
    }
}

/// The `n`th Sunday of `month` in `year`, in days since the Unix epoch.
fn nth_sunday(year: i64, month: u32, n: i64) -> i64 {
    let first = days_from_civil(year, month, 1);
    let to_sunday = (7 - i64::from(weekday(first))) % 7;
    first + to_sunday + 7 * (n - 1)
}

/// The last Sunday of `month` in `year`, in days since the Unix epoch.
fn last_sunday(year: i64, month: u32) -> i64 {
    let next = if month == 12 {
        days_from_civil(year + 1, 1, 1)
    } else {
        days_from_civil(year, month + 1, 1)
    };
    let last = next - 1;
    last - i64::from(weekday(last))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(year: i64, month: u32, day: u32, hour: i64, minute: i64) -> i64 {
        days_from_civil(year, month, day) * DAY + hour * HOUR + minute * 60_000
    }

    fn hours(place: &str, at: i64) -> f64 {
        find(place).unwrap().offset(at) as f64 / HOUR as f64
    }

    #[test]
    fn places_without_daylight_saving_keep_their_offset() {
        assert_eq!(hours("Tokyo", utc(2026, 1, 15, 0, 0)), 9.0);
        assert_eq!(hours("tokyo", utc(2026, 7, 15, 0, 0)), 9.0);
        assert_eq!(hours("Mumbai", utc(2026, 7, 15, 0, 0)), 5.5);
        assert_eq!(hours("kathmandu", utc(2026, 7, 15, 0, 0)), 5.75);
        assert_eq!(hours("Phoenix", utc(2026, 7, 15, 0, 0)), -7.0);
    }

    #[test]
    fn the_united_states_change_at_two_in_the_morning_local_time() {
        // 2026: from 8 March to 1 November.
        assert_eq!(hours("new york", utc(2026, 3, 8, 6, 59)), -5.0);
        assert_eq!(hours("new york", utc(2026, 3, 8, 7, 0)), -4.0);
        assert_eq!(hours("new york", utc(2026, 11, 1, 5, 59)), -4.0);
        assert_eq!(hours("new york", utc(2026, 11, 1, 6, 0)), -5.0);
        assert_eq!(hours("SF", utc(2026, 7, 1, 0, 0)), -7.0);
    }

    #[test]
    fn europe_changes_at_one_in_the_morning_utc() {
        // 2026: from 29 March to 25 October.
        assert_eq!(hours("London", utc(2026, 3, 29, 0, 59)), 0.0);
        assert_eq!(hours("London", utc(2026, 3, 29, 1, 0)), 1.0);
        assert_eq!(hours("Paris", utc(2026, 10, 25, 0, 59)), 2.0);
        assert_eq!(hours("Paris", utc(2026, 10, 25, 1, 0)), 1.0);
    }

    #[test]
    fn the_south_keeps_daylight_saving_over_the_new_year() {
        // Sydney 2026: until 5 April, from 4 October.
        assert_eq!(hours("sydney", utc(2026, 1, 15, 0, 0)), 11.0);
        assert_eq!(hours("sydney", utc(2026, 7, 15, 0, 0)), 10.0);
        assert_eq!(hours("sydney", utc(2026, 10, 3, 15, 59)), 10.0);
        assert_eq!(hours("sydney", utc(2026, 10, 3, 16, 0)), 11.0);
        assert_eq!(hours("auckland", utc(2026, 1, 15, 0, 0)), 13.0);
        assert_eq!(hours("auckland", utc(2026, 7, 15, 0, 0)), 12.0);
        assert_eq!(hours("brisbane", utc(2026, 1, 15, 0, 0)), 10.0);
    }

    #[test]
    fn unknown_places_have_no_zone() {
        assert_eq!(find("atlantis"), None);
        assert_eq!(find("santiago"), None);
    }
}
