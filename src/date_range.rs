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

impl DateRange {
    pub const ALL_TIME: Self = Self { from_date: None, to_date: None };

    pub fn from_strs(from_date: &str, to_date: &str) -> anyhow::Result<Self> {
        let from_date = match from_date {
            "forever" => None,
            "ytd" => Some(
                Utc.from_utc_datetime(&NaiveDateTime::new(
                    NaiveDate::from_ymd_opt(Utc::now().year(), 1, 1)
                        .expect("Jan 1 should always be valid in the current year."),
                    NaiveTime::MIN,
                )),
            ),
            "today" => Some(Utc::now()),
            date_string => {
                let date = NaiveDate::parse_from_str(date_string, "%Y-%m-%d")
                    .map(|date| Utc.from_utc_datetime(&NaiveDateTime::new(date, NaiveTime::MIN)));
                if let Ok(date) = date {
                    Some(date)
                } else {
                    bail!("Invalid date format \"{date_string}\". Use 'forever', 'ytd', 'today', or '%Y-%m-%d'");
                }
            }
        };
        let to_date = match to_date {
            "forever" => None,
            "today" => Some(Utc::now()),
            date_string => {
                let date = NaiveDate::parse_from_str(date_string, "%Y-%m-%d")
                    .map(|date| Utc.from_utc_datetime(&NaiveDateTime::new(date, NaiveTime::MIN)));
                if let Ok(date) = date {
                    Some(date)
                } else {
                    bail!("Invalid date format \"{date_string}\". Use 'forever', 'ytd', 'today', or '%Y-%m-%d'");
                }
            }
        };

        Ok(Self { from_date, to_date })
    }
}
