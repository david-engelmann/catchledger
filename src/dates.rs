//! Timestamp reading and rendering.
//!
//! The reading reproduces the grammar of the Python `fromisoformat`
//! on the building machine's runtime, as the contract probes record
//! it. Extended and basic date and time forms and week dates stand,
//! ordinal dates do not, any single character may separate the date
//! from the time, a fraction after any time component is a fraction
//! of a second, and basic time digits are read in pairs with a lone
//! trailing digit dropped. Instants are microseconds since the Unix
//! epoch, already normalized to UTC.

use crate::errors::CatchledgerError;
use crate::jsonvalue::JsonValue;

const MICROS_PER_DAY: i128 = 86_400_000_000;

/// Whether a character is whitespace to the Python string strip.
pub fn py_whitespace(ch: char) -> bool {
    ch.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&ch)
}

/// Strip a string the way the Python string strip does.
pub fn py_strip(text: &str) -> &str {
    text.trim_matches(py_whitespace)
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// Days since the Unix epoch for a civil date.
pub fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let adjusted = if month <= 2 { year - 1 } else { year };
    let era = adjusted.div_euclid(400);
    let year_of_era = adjusted - era * 400;
    let shifted_month = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The civil date for a day count since the Unix epoch.
pub fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// Weekday with Monday at zero for a day count since the epoch.
fn weekday_mon0(days: i64) -> i64 {
    (days + 3).rem_euclid(7)
}

fn iso_weeks_in_year(year: i64) -> i64 {
    let jan1 = weekday_mon0(days_from_civil(year, 1, 1));
    if jan1 == 3 || (jan1 == 2 && is_leap_year(year)) {
        53
    } else {
        52
    }
}

struct Parsed {
    days: i64,
    hour: i64,
    minute: i64,
    second: i64,
    micros: i64,
    offset_micros: Option<i128>,
}

struct Cursor {
    chars: Vec<char>,
    pos: usize,
}

impl Cursor {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    fn advance(&mut self) {
        self.pos += 1;
    }

    fn take_digits(&mut self, count: usize) -> Option<i64> {
        let mut value: i64 = 0;
        for index in 0..count {
            let ch = self.peek_at(index)?;
            if !ch.is_ascii_digit() {
                return None;
            }
            value = value * 10 + i64::from(ch as u8 - b'0');
        }
        self.pos += count;
        Some(value)
    }

    fn digit_run(&self) -> usize {
        let mut length = 0;
        while let Some(ch) = self.peek_at(length) {
            if ch.is_ascii_digit() {
                length += 1;
            } else {
                break;
            }
        }
        length
    }

    /// Drop one trailing digit that stands alone before a terminator.
    fn drop_lone_digit(&mut self) {
        if let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                match self.peek_at(1) {
                    None => self.advance(),
                    Some('Z') | Some('+') | Some('-') => self.advance(),
                    _ => {}
                }
            }
        }
    }

    /// Read a dotted fraction as microseconds of one second.
    fn dotted_fraction_micros(&mut self) -> Option<i64> {
        self.fraction_micros(false)
    }

    /// Read a fraction as microseconds of one second.
    ///
    /// The cursor stands on the fraction separator. A separator
    /// standing at the end of a time an offset marker follows reads
    /// as a fraction of no digits when `empty_before_offset` holds,
    /// as the Python reading reads it, and refuses otherwise.
    fn fraction_micros(&mut self, empty_before_offset: bool) -> Option<i64> {
        self.advance();
        let run = self.digit_run();
        if run == 0 {
            if empty_before_offset && matches!(self.peek(), Some('+') | Some('-')) {
                return Some(0);
            }
            return None;
        }
        let mut micros: i64 = 0;
        let take = run.min(6);
        for index in 0..take {
            let ch = self.peek_at(index)?;
            micros = micros * 10 + i64::from(ch as u8 - b'0');
        }
        for _ in take..6 {
            micros *= 10;
        }
        self.pos += run;
        Some(micros)
    }
}

fn calendar_days(year: i64, month: i64, day: i64) -> Option<i64> {
    if !(1..=9999).contains(&year) || !(1..=12).contains(&month) {
        return None;
    }
    if day < 1 || day > days_in_month(year, month) {
        return None;
    }
    Some(days_from_civil(year, month, day))
}

fn week_days(year: i64, week: i64, weekday: i64) -> Option<i64> {
    if !(1..=9999).contains(&year) {
        return None;
    }
    if week < 1 || week > iso_weeks_in_year(year) || !(1..=7).contains(&weekday) {
        return None;
    }
    let jan4 = days_from_civil(year, 1, 4);
    let week_one_monday = jan4 - weekday_mon0(jan4);
    Some(week_one_monday + (week - 1) * 7 + (weekday - 1))
}

fn parse_iso(cursor: &mut Cursor) -> Option<Parsed> {
    let year = cursor.take_digits(4)?;
    let days = if cursor.peek() == Some('-') {
        cursor.advance();
        if cursor.peek() == Some('W') {
            cursor.advance();
            let week = cursor.take_digits(2)?;
            if cursor.peek() != Some('-') {
                return None;
            }
            cursor.advance();
            let weekday = cursor.take_digits(1)?;
            week_days(year, week, weekday)?
        } else {
            let month = cursor.take_digits(2)?;
            if cursor.peek() != Some('-') {
                return None;
            }
            cursor.advance();
            let day = cursor.take_digits(2)?;
            calendar_days(year, month, day)?
        }
    } else if cursor.peek() == Some('W') {
        cursor.advance();
        let week = cursor.take_digits(2)?;
        let weekday = cursor.take_digits(1)?;
        week_days(year, week, weekday)?
    } else {
        let month = cursor.take_digits(2)?;
        let day = cursor.take_digits(2)?;
        calendar_days(year, month, day)?
    };

    let mut parsed = Parsed {
        days,
        hour: 0,
        minute: 0,
        second: 0,
        micros: 0,
        offset_micros: None,
    };
    if cursor.peek().is_none() {
        return Some(parsed);
    }
    // Exactly one separator character stands between date and time,
    // and it may be any character at all.
    cursor.advance();

    if cursor.peek_at(2) == Some(':') {
        parsed.hour = cursor.take_digits(2)?;
        cursor.drop_lone_digit();
        if cursor.peek() == Some(':') {
            cursor.advance();
            parsed.minute = cursor.take_digits(2)?;
            cursor.drop_lone_digit();
            if cursor.peek() == Some(':') {
                cursor.advance();
                parsed.second = cursor.take_digits(2)?;
                cursor.drop_lone_digit();
            }
        }
        if matches!(cursor.peek(), Some('.') | Some(',') | Some(':')) {
            parsed.micros = cursor.fraction_micros(true)?;
        }
    } else {
        let start = cursor.pos;
        let run = cursor.digit_run();
        if run < 2 {
            return None;
        }
        let pair = |offset: usize| -> i64 {
            let a = cursor.chars[start + offset] as i64 - '0' as i64;
            let b = cursor.chars[start + offset + 1] as i64 - '0' as i64;
            a * 10 + b
        };
        parsed.hour = pair(0);
        let mut index = 2;
        if run - index >= 2 {
            parsed.minute = pair(index);
            index += 2;
        }
        if run - index >= 2 {
            parsed.second = pair(index);
            index += 2;
        }
        let mut fraction = String::new();
        while run - index >= 2 && fraction.len() < 6 {
            fraction.push(cursor.chars[start + index]);
            fraction.push(cursor.chars[start + index + 1]);
            index += 2;
        }
        if !fraction.is_empty() {
            let digits: i64 = fraction.parse().ok()?;
            parsed.micros = digits * 10i64.pow((6 - fraction.len()) as u32);
        }
        let lone_digit_dropped = run - index == 1;
        cursor.pos = start + run;
        if matches!(cursor.peek(), Some('.') | Some(',')) {
            if !fraction.is_empty() || lone_digit_dropped {
                return None;
            }
            parsed.micros = cursor.fraction_micros(true)?;
        }
    }
    if parsed.hour > 23 || parsed.minute > 59 || parsed.second > 59 {
        return None;
    }

    match cursor.peek() {
        None => {}
        Some('Z') => {
            cursor.advance();
            parsed.offset_micros = Some(0);
        }
        Some(sign @ ('+' | '-')) => {
            cursor.advance();
            let offset_hour = cursor.take_digits(2)?;
            let mut offset_minute = 0;
            let mut offset_second = 0;
            let mut colon_form = false;
            let mut read_second = false;
            if cursor.peek() == Some(':') {
                colon_form = true;
                cursor.advance();
                offset_minute = cursor.take_digits(2)?;
                if cursor.peek() == Some(':') {
                    cursor.advance();
                    offset_second = cursor.take_digits(2)?;
                    read_second = true;
                }
            } else if matches!(cursor.peek(), Some(ch) if ch.is_ascii_digit()) {
                offset_minute = cursor.take_digits(2)?;
                if matches!(cursor.peek(), Some(ch) if ch.is_ascii_digit()) {
                    offset_second = cursor.take_digits(2)?;
                    read_second = true;
                }
            }
            // The components are summed rather than range checked
            // one by one, as the Python reading sums them, and only
            // the total is bounded, at a full day.
            let whole_seconds = (offset_hour * 60 + offset_minute) * 60 + offset_second;
            let mut offset_micros: i128 = whole_seconds as i128 * 1_000_000;
            let fraction_separator = match cursor.peek() {
                Some('.') | Some(',') => true,
                // A colon standing after the seconds of the colon
                // form opens the offset fraction, as it does in the
                // Python reading, where the basic form refuses it.
                Some(':') => colon_form && read_second,
                _ => false,
            };
            if fraction_separator {
                let fraction_micros = cursor.dotted_fraction_micros()?;
                // The Python reading drops an offset fraction when
                // the whole second part of the offset is zero.
                if whole_seconds != 0 {
                    offset_micros += i128::from(fraction_micros);
                }
            }
            if offset_micros >= MICROS_PER_DAY {
                return None;
            }
            parsed.offset_micros = Some(if sign == '-' {
                -offset_micros
            } else {
                offset_micros
            });
        }
        _ => return None,
    }
    if cursor.peek().is_some() {
        return None;
    }
    Some(parsed)
}

/// Read a timestamp value and normalize it to UTC microseconds.
///
/// Mirrors `parse_timestamp` in the Python models module, including
/// its two refusal messages with the field name standing in them.
pub fn parse_timestamp(value: &JsonValue, field: &str) -> Result<i128, CatchledgerError> {
    let iso_message = || CatchledgerError::new(format!("{field} must be an ISO 8601 timestamp"));
    let text = match value {
        JsonValue::Text(text) => text,
        _ => return Err(iso_message()),
    };
    let stripped = py_strip(text);
    if stripped.is_empty() {
        return Err(iso_message());
    }
    let replaced = match stripped.strip_suffix('Z') {
        Some(head) => format!("{head}+00:00"),
        None => stripped.to_string(),
    };
    let mut cursor = Cursor {
        chars: replaced.chars().collect(),
        pos: 0,
    };
    let parsed = parse_iso(&mut cursor).ok_or_else(iso_message)?;
    match parsed.offset_micros {
        None => Err(CatchledgerError::new(format!(
            "{field} must carry a timezone offset"
        ))),
        Some(offset) => {
            let local = parsed.days as i128 * MICROS_PER_DAY
                + ((parsed.hour * 60 + parsed.minute) * 60 + parsed.second) as i128 * 1_000_000
                + parsed.micros as i128;
            Ok(local - offset)
        }
    }
}

/// Render UTC microseconds the way the Python `format_timestamp`
/// renders them, with a trailing Z and a six digit fraction printed
/// exactly when the fraction is nonzero.
pub fn format_timestamp(micros: i128) -> String {
    let days = micros.div_euclid(MICROS_PER_DAY);
    let rest = micros.rem_euclid(MICROS_PER_DAY);
    let (year, month, day) = civil_from_days(days as i64);
    let hour = rest / 3_600_000_000;
    let minute = (rest / 60_000_000) % 60;
    let second = (rest / 1_000_000) % 60;
    let fraction = rest % 1_000_000;
    if fraction != 0 {
        format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{fraction:06}Z")
    } else {
        format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
    }
}
