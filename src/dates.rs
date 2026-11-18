use chrono::{Datelike, Local, NaiveDate};

use crate::error::{msg, Result};

const MONTHS: [&str; 12] = [
    "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic",
];
const WEEKDAYS: [&str; 7] = ["dom", "lun", "mar", "mié", "jue", "vie", "sáb"];

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

pub fn from_api(value: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(value.trim(), "%d-%b-%Y")
        .map_err(|_| msg(format!("fecha inválida: {value:?}")))
}

pub fn parse_any(value: &str) -> Result<NaiveDate> {
    let value = value.trim();
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .or_else(|_| from_api(value))
        .map_err(|_| msg(format!("periodo inválido: {value} · usa 2026-11-01")))
}

#[cfg(test)]
pub fn to_api(date: NaiveDate) -> String {
    date.format("%d-%b-%Y").to_string()
}

pub fn ymd(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

#[cfg(test)]
pub fn day_name(date: NaiveDate) -> &'static str {
    ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
        [date.weekday().num_days_from_sunday() as usize]
}

pub fn day_month(date: NaiveDate) -> String {
    format!("{:02} {}", date.day(), MONTHS[date.month0() as usize])
}

pub fn weekday_short(date: NaiveDate) -> &'static str {
    WEEKDAYS[date.weekday().num_days_from_sunday() as usize]
}

pub fn day_month_weekday(date: NaiveDate) -> String {
    format!("{} {}", day_month(date), weekday_short(date))
}

/// Rango corto: "08 nov – 14 nov" (con año solo si no es el actual).
pub fn range(start: NaiveDate, end: NaiveDate, today: NaiveDate) -> String {
    let suffix = if start.year() == today.year() {
        String::new()
    } else {
        format!(" {:02}", start.year() % 100)
    };
    let left = if start.month() == end.month() {
        format!("{:02}", start.day())
    } else {
        day_month(start)
    };
    format!("{} – {}{}", left, day_month(end), suffix)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn parses_api_dates() {
        assert_eq!(from_api("01-Nov-2026").unwrap(), date(2026, 11, 1));
        assert_eq!(from_api("22-Oct-2026").unwrap(), date(2026, 10, 22));
        assert!(from_api("2026-11-01").is_err());
    }

    #[test]
    fn formats_api_dates() {
        assert_eq!(to_api(date(2026, 10, 22)), "22-Oct-2026");
        assert_eq!(to_api(date(2026, 1, 5)), "05-Jan-2026");
    }

    #[test]
    fn parses_any_format() {
        assert_eq!(parse_any("2026-11-01").unwrap(), date(2026, 11, 1));
        assert_eq!(parse_any("01-Nov-2026").unwrap(), date(2026, 11, 1));
        assert!(parse_any("nope").is_err());
    }

    #[test]
    fn formats_ranges() {
        let today = date(2026, 11, 17);
        assert_eq!(
            range(date(2026, 11, 8), date(2026, 11, 14), today),
            "08 – 14 nov"
        );
        assert_eq!(
            range(date(2026, 10, 25), date(2026, 10, 31), today),
            "25 – 31 oct"
        );
        assert_eq!(
            range(date(2025, 12, 28), date(2026, 1, 3), today),
            "28 dic – 03 ene 25"
        );
    }

    #[test]
    fn day_names() {
        assert_eq!(day_name(date(2026, 11, 17)), "Tue");
        assert_eq!(weekday_short(date(2026, 11, 17)), "mar");
        assert_eq!(day_month_weekday(date(2026, 11, 9)), "09 nov lun");
    }
}
