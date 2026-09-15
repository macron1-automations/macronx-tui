const MONTHS: [(&str, &str, &str); 12] = [
    ("Jan", "January", "01"),
    ("Feb", "February", "02"),
    ("Mar", "March", "03"),
    ("Apr", "April", "04"),
    ("May", "May", "05"),
    ("Jun", "June", "06"),
    ("Jul", "July", "07"),
    ("Aug", "August", "08"),
    ("Sep", "September", "09"),
    ("Oct", "October", "10"),
    ("Nov", "November", "11"),
    ("Dec", "December", "12"),
];

struct Timestamp<'a> {
    date: &'a str,
    year: &'a str,
    month: &'a str,
    day: &'a str,
    hhmm: Option<&'a str>,
}

impl<'a> Timestamp<'a> {
    fn parse(s: &'a str) -> Option<Self> {
        if s.len() < 10 {
            return None;
        }
        let date = s.get(..10)?;
        let mut parts = date.split('-');
        let year = parts.next()?;
        let month = parts.next()?;
        let day = parts.next()?;
        if parts.next().is_some() {
            return None;
        }
        let hhmm = if s.len() >= 16 { s.get(11..16) } else { None };
        Some(Self {
            date,
            year,
            month,
            day,
            hhmm,
        })
    }

    fn month_short(&self) -> &str {
        month_lookup(self.month)
            .map(|i| MONTHS[i].0)
            .unwrap_or(self.month)
    }

    fn month_long(&self) -> &str {
        month_lookup(self.month)
            .map(|i| MONTHS[i].1)
            .unwrap_or(self.month)
    }
}

fn month_lookup(month: &str) -> Option<usize> {
    MONTHS.iter().position(|(.., num)| *num == month)
}

pub fn date_short(s: &str) -> String {
    match Timestamp::parse(s) {
        Some(ts) => format!(
            "{} {}, {}",
            ts.month_short(),
            ts.day.trim_start_matches('0'),
            ts.year
        ),
        None => s.to_string(),
    }
}

pub fn datetime_long(s: &str) -> String {
    match Timestamp::parse(s) {
        Some(ts) if ts.hhmm.is_some() => format!(
            "{} {}, {} at {} UTC",
            ts.month_long(),
            ts.day.trim_start_matches('0'),
            ts.year,
            ts.hhmm.unwrap()
        ),
        _ => s.to_string(),
    }
}

pub fn datetime_compact(s: &str) -> String {
    match Timestamp::parse(s) {
        Some(ts) if ts.hhmm.is_some() => format!("{} {}", ts.date, ts.hhmm.unwrap()),
        _ => s.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TS: &str = "2026-06-12T20:40:00.000Z";

    #[test]
    fn formats_documented_examples() {
        assert_eq!(date_short(TS), "Jun 12, 2026");
        assert_eq!(datetime_long(TS), "June 12, 2026 at 20:40 UTC");
        assert_eq!(datetime_compact(TS), "2026-06-12 20:40");
    }

    #[test]
    fn trims_leading_zero_from_day() {
        assert_eq!(date_short("2026-06-02T09:05:00Z"), "Jun 2, 2026");
    }

    #[test]
    fn falls_back_to_raw_for_unparseable_input() {
        assert_eq!(date_short("n/a"), "n/a");
        assert_eq!(datetime_long(""), "");
        assert_eq!(datetime_compact("garbage"), "garbage");
        assert_eq!(date_short("2026-06-12"), "Jun 12, 2026");
        assert_eq!(datetime_long("2026-06-12"), "2026-06-12");
        assert_eq!(datetime_compact("2026-06-12"), "2026-06-12");
    }

    #[test]
    fn unknown_month_passes_through_verbatim() {
        assert_eq!(date_short("2026-13-05T00:00Z"), "13 5, 2026");
        assert_eq!(
            datetime_long("2026-13-05T00:00Z"),
            "13 5, 2026 at 00:00 UTC"
        );
    }
}
