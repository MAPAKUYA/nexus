//! Минимальная арифметика григорианских дат без внешних зависимостей.
//! Все даты — локальные, в формате `YYYY-MM-DD`; строки сравниваются лексикографически.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Date {
    pub y: i32,
    pub m: u32,
    pub d: u32,
}

const MIN_YEAR: i32 = 1900;
const MAX_YEAR: i32 = 9999;

pub fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

pub fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if is_leap(y) { 29 } else { 28 },
        _ => 0,
    }
}

// Алгоритмы Говарда Хиннанта (days_from_civil / civil_from_days).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

impl Date {
    /// Строго `YYYY-MM-DD`, существующая календарная дата в годах 1900–9999.
    pub fn parse(s: &str) -> Option<Date> {
        let b = s.as_bytes();
        if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
            return None;
        }
        if !b.iter().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit()) {
            return None;
        }
        let y: i32 = s[0..4].parse().ok()?;
        let m: u32 = s[5..7].parse().ok()?;
        let d: u32 = s[8..10].parse().ok()?;
        if !(MIN_YEAR..=MAX_YEAR).contains(&y) || !(1..=12).contains(&m) || d < 1 || d > days_in_month(y, m) {
            return None;
        }
        Some(Date { y, m, d })
    }

    fn to_days(self) -> i64 {
        days_from_civil(self.y as i64, self.m as i64, self.d as i64)
    }

    fn from_days(z: i64) -> Date {
        let (y, m, d) = civil_from_days(z);
        Date { y: y as i32, m: m as u32, d: d as u32 }
    }

    pub fn add_days(self, n: i64) -> Option<Date> {
        let r = Date::from_days(self.to_days() + n);
        (MIN_YEAR..=MAX_YEAR).contains(&r.y).then_some(r)
    }

    /// Число дней от `self` до `other` (отрицательное, если `other` раньше).
    pub fn days_until(self, other: Date) -> i64 {
        other.to_days() - self.to_days()
    }

    /// Понедельник = 0 … воскресенье = 6.
    pub fn weekday_mon0(self) -> u32 {
        // 1970-01-01 — четверг (индекс 3).
        (self.to_days() + 3).rem_euclid(7) as u32
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.y, self.m, self.d)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    Day,
    Week,
    Month,
    Year,
    All,
}

impl Period {
    pub fn parse(s: &str) -> Option<Period> {
        Some(match s {
            "day" => Period::Day,
            "week" => Period::Week,
            "month" => Period::Month,
            "year" => Period::Year,
            "all" => Period::All,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Period::Day => "day",
            Period::Week => "week",
            Period::Month => "month",
            Period::Year => "year",
            Period::All => "all",
        }
    }

    /// Границы периода (включительно). Неделя начинается с понедельника.
    pub fn range(self, a: Date) -> (String, String) {
        match self {
            Period::Day => (a.to_string(), a.to_string()),
            Period::Week => {
                let mon = a.add_days(-(a.weekday_mon0() as i64)).unwrap_or(a);
                let sun = mon.add_days(6).unwrap_or(mon);
                (mon.to_string(), sun.to_string())
            }
            Period::Month => (
                Date { y: a.y, m: a.m, d: 1 }.to_string(),
                Date { y: a.y, m: a.m, d: days_in_month(a.y, a.m) }.to_string(),
            ),
            Period::Year => (format!("{:04}-01-01", a.y), format!("{:04}-12-31", a.y)),
            Period::All => ("0000-01-01".into(), "9999-12-31".into()),
        }
    }

    /// Сдвиг «опорной» даты на один период назад/вперёд.
    pub fn shift(self, a: Date, delta: i32) -> Option<Date> {
        match self {
            Period::Day => a.add_days(delta as i64),
            Period::Week => a.add_days(7 * delta as i64),
            Period::Month => {
                let total = a.y * 12 + (a.m as i32 - 1) + delta;
                let (y, m) = (total.div_euclid(12), (total.rem_euclid(12) + 1) as u32);
                (MIN_YEAR..=MAX_YEAR)
                    .contains(&y)
                    .then(|| Date { y, m, d: a.d.min(days_in_month(y, m)) })
            }
            Period::Year => {
                let y = a.y + delta;
                (MIN_YEAR..=MAX_YEAR)
                    .contains(&y)
                    .then(|| Date { y, m: a.m, d: a.d.min(days_in_month(y, a.m)) })
            }
            Period::All => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        Date::parse(s).unwrap()
    }

    #[test]
    fn parse_accepts_valid_and_rejects_invalid() {
        assert!(Date::parse("2026-10-30").is_some());
        assert!(Date::parse("2024-02-29").is_some());
        for bad in ["2025-02-29", "2026-13-01", "2026-00-10", "2026-04-31", "26-10-30", "2026/10/30", "2026-1-30", "", "abcd-ef-gh", "2026-10-30 ", "1899-12-31", "x'; DROP--"] {
            assert!(Date::parse(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn weekday_is_correct_for_known_dates() {
        assert_eq!(d("1970-01-01").weekday_mon0(), 3); // четверг
        assert_eq!(d("2026-10-30").weekday_mon0(), 4); // пятница
        assert_eq!(d("2024-02-29").weekday_mon0(), 3); // четверг
        assert_eq!(d("2000-01-01").weekday_mon0(), 5); // суббота
    }

    #[test]
    fn add_days_crosses_month_year_and_leap_boundaries() {
        assert_eq!(d("2026-12-31").add_days(1).unwrap().to_string(), "2027-01-01");
        assert_eq!(d("2024-02-28").add_days(1).unwrap().to_string(), "2024-02-29");
        assert_eq!(d("2025-02-28").add_days(1).unwrap().to_string(), "2025-03-01");
        assert_eq!(d("2026-01-01").add_days(-1).unwrap().to_string(), "2025-12-31");
        assert_eq!(d("2026-10-30").add_days(-365).unwrap().to_string(), "2025-10-30");
    }

    #[test]
    fn week_starts_on_monday() {
        // пятница 30.10.2026 -> неделя 26.10 – 01.11
        assert_eq!(Period::Week.range(d("2026-10-30")), ("2026-10-26".into(), "2026-11-01".into()));
        // понедельник и воскресенье той же недели дают ту же неделю
        assert_eq!(Period::Week.range(d("2026-10-26")), Period::Week.range(d("2026-11-01")));
    }

    #[test]
    fn month_year_and_all_ranges() {
        assert_eq!(Period::Month.range(d("2024-02-10")), ("2024-02-01".into(), "2024-02-29".into()));
        assert_eq!(Period::Month.range(d("2026-12-31")), ("2026-12-01".into(), "2026-12-31".into()));
        assert_eq!(Period::Year.range(d("2026-06-15")), ("2026-01-01".into(), "2026-12-31".into()));
        let (a, b) = Period::All.range(d("2026-06-15"));
        assert!("2026-06-15" > a.as_str() && "2026-06-15" < b.as_str());
    }

    #[test]
    fn shift_clamps_day_and_crosses_years() {
        assert_eq!(Period::Month.shift(d("2026-01-31"), 1).unwrap().to_string(), "2026-02-28");
        assert_eq!(Period::Month.shift(d("2026-01-15"), -1).unwrap().to_string(), "2025-12-15");
        assert_eq!(Period::Month.shift(d("2026-12-15"), 1).unwrap().to_string(), "2027-01-15");
        assert_eq!(Period::Year.shift(d("2024-02-29"), 1).unwrap().to_string(), "2025-02-28");
        assert_eq!(Period::Week.shift(d("2026-10-30"), -1).unwrap().to_string(), "2026-10-23");
        assert_eq!(Period::Day.shift(d("2026-03-01"), -1).unwrap().to_string(), "2026-02-28");
        assert!(Period::All.shift(d("2026-03-01"), 1).is_none());
        assert!(Period::Year.shift(d("1900-05-05"), -1).is_none());
    }

    #[test]
    fn every_day_of_four_years_roundtrips() {
        let mut cur = d("2023-01-01");
        for _ in 0..(365 * 4 + 1) {
            assert_eq!(Date::parse(&cur.to_string()), Some(cur));
            let next = cur.add_days(1).unwrap();
            assert_eq!(next.add_days(-1).unwrap(), cur);
            assert_eq!((next.weekday_mon0() + 6) % 7, cur.weekday_mon0());
            cur = next;
        }
    }

    #[test]
    fn period_string_roundtrip() {
        for p in [Period::Day, Period::Week, Period::Month, Period::Year, Period::All] {
            assert_eq!(Period::parse(p.as_str()), Some(p));
        }
        assert!(Period::parse("decade").is_none());
    }
}
