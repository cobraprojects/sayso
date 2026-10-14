//! Dates in the words and the order of the interface language. Each catalog
//! has the patterns and the names of the months and the days, so a
//! translator controls "September 28", "28 de septiembre", and "9月28日".

use crate::t;
use chrono::{Datelike, Weekday};

/// "Sep 28".
pub fn day_month_short(date: &impl Datelike) -> String {
    t!("date.pattern.day_month_short", month = month_short(date.month()), day = date.day())
}

/// "September 28".
pub fn day_month(date: &impl Datelike) -> String {
    t!("date.pattern.day_month", month = month(date.month()), day = date.day())
}

/// "Monday, September 28".
pub fn weekday_day_month(date: &impl Datelike) -> String {
    t!("date.pattern.weekday_day_month", weekday = weekday(date.weekday()), month = month(date.month()), day = date.day())
}

/// "September 28, 2026".
pub fn day_month_year(date: &impl Datelike) -> String {
    t!("date.pattern.day_month_year", month = month(date.month()), day = date.day(), year = date.year())
}

/// "Today".
pub fn today() -> String {
    t!("date.today")
}

/// "Yesterday".
pub fn yesterday() -> String {
    t!("date.yesterday")
}

/// The name of a month, 1 to 12: "September".
pub fn month(m: u32) -> String {
    match m {
        1 => t!("date.month.january"),
        2 => t!("date.month.february"),
        3 => t!("date.month.march"),
        4 => t!("date.month.april"),
        5 => t!("date.month.may"),
        6 => t!("date.month.june"),
        7 => t!("date.month.july"),
        8 => t!("date.month.august"),
        9 => t!("date.month.september"),
        10 => t!("date.month.october"),
        11 => t!("date.month.november"),
        _ => t!("date.month.december"),
    }
}

/// The short name of a month, 1 to 12: "Sep".
pub fn month_short(m: u32) -> String {
    match m {
        1 => t!("date.month_short.january"),
        2 => t!("date.month_short.february"),
        3 => t!("date.month_short.march"),
        4 => t!("date.month_short.april"),
        5 => t!("date.month_short.may"),
        6 => t!("date.month_short.june"),
        7 => t!("date.month_short.july"),
        8 => t!("date.month_short.august"),
        9 => t!("date.month_short.september"),
        10 => t!("date.month_short.october"),
        11 => t!("date.month_short.november"),
        _ => t!("date.month_short.december"),
    }
}

/// The name of a day: "Monday".
pub fn weekday(d: Weekday) -> String {
    match d {
        Weekday::Mon => t!("date.weekday.monday"),
        Weekday::Tue => t!("date.weekday.tuesday"),
        Weekday::Wed => t!("date.weekday.wednesday"),
        Weekday::Thu => t!("date.weekday.thursday"),
        Weekday::Fri => t!("date.weekday.friday"),
        Weekday::Sat => t!("date.weekday.saturday"),
        Weekday::Sun => t!("date.weekday.sunday"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn english_dates() {
        crate::set_locale("en");
        let d = NaiveDate::from_ymd_opt(2026, 9, 28).unwrap();
        assert_eq!(day_month_short(&d), "Sep 28");
        assert_eq!(day_month(&d), "September 28");
        assert_eq!(weekday_day_month(&d), "Monday, September 28");
        assert_eq!(day_month_year(&d), "September 28, 2026");
    }
}
