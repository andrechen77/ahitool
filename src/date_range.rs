use anyhow::bail;
use chrono::{Datelike as _, NaiveDate, NaiveDateTime, NaiveTime, TimeZone as _, Utc};

use crate::jobs::Timestamp;

pub struct DateRange {
    /// The start of the date range. If `None`, the range is from the beginning
    /// of time.
    pub from_date: Option<Timestamp>,
    /// The end of the date range. If `None`, the range is until the end of
    /// time.
    pub to_date: Option<Timestamp>,
}

const DATE_FORMATS: [&str; 2] = ["%m/%d/%y", "%m/%d/%Y"];

const MAX_TIME: NaiveTime = NaiveTime::from_hms_opt(11, 59, 59).unwrap();

impl DateRange {
    pub const ALL_TIME: Self = Self { from_date: None, to_date: None };

    pub fn last_year() -> Self {
        let now = Utc::now();
        let last_year = now.year() - 1;
        let from_date = Utc.from_utc_datetime(&NaiveDateTime::new(
            NaiveDate::from_ymd_opt(last_year, 1, 1).expect("Jan 1 should always be valid"),
            NaiveTime::MIN,
        ));
        let to_date = Utc.from_utc_datetime(&NaiveDateTime::new(
            NaiveDate::from_ymd_opt(last_year, 12, 31).expect("Dec 31 should always be valid"),
            MAX_TIME,
        ));
        Self { from_date: Some(from_date), to_date: Some(to_date) }
    }

    pub fn year_to_date() -> Self {
        let now = Utc::now();
        let current_year = now.year();
        let from_date = Utc.from_utc_datetime(&NaiveDateTime::new(
            NaiveDate::from_ymd_opt(current_year - 1, 1, 1).expect("Jan 1 should always be valid"),
            NaiveTime::MIN,
        ));
        Self { from_date: Some(from_date), to_date: Some(now) }
    }

    pub fn from_strs(from_date: &str, to_date: &str) -> anyhow::Result<Self> {
        let from_date = match from_date {
            "Forever" => None,
            "Start-of-year" => Some(start_of_year()),
            "Today" => Some(Utc::now()),
            date_string => {
                let date = DATE_FORMATS
                    .iter()
                    .find_map(|format| NaiveDate::parse_from_str(date_string, format).ok())
                    .map(|date| Utc.from_utc_datetime(&NaiveDateTime::new(date, NaiveTime::MIN)));
                if let Some(date) = date {
                    Some(date)
                } else {
                    bail!("invalid date format \"{date_string}\". Use 'Forever', 'Start-of-year', 'Today', or '{}'", DATE_FORMATS.join(" or "));
                }
            }
        };
        let to_date = match to_date {
            "Forever" => None,
            "Start-of-year" => Some(start_of_year()),
            "Today" => Some(Utc::now()),
            date_string => {
                let date = DATE_FORMATS
                    .iter()
                    .find_map(|format| NaiveDate::parse_from_str(date_string, format).ok())
                    .map(|date| Utc.from_utc_datetime(&NaiveDateTime::new(date, MAX_TIME)));
                if let Some(date) = date {
                    Some(date)
                } else {
                    bail!("invalid date format \"{date_string}\". Use 'Forever', 'Start-of-year', 'Today', or '{}'", DATE_FORMATS.join(" or "));
                }
            }
        };

        Ok(Self { from_date, to_date })
    }
}

pub fn start_of_year() -> Timestamp {
    Utc.from_utc_datetime(&NaiveDateTime::new(
        NaiveDate::from_ymd_opt(Utc::now().year(), 1, 1)
            .expect("Jan 1 should always be valid in the current year."),
        NaiveTime::MIN,
    ))
}
