// SPDX-License-Identifier: AGPL-3.0-only
//! `Date.parse(text)` as V8 reads it (node 22): the ISO format first, then the legacy reader that takes a date with a
//! space where the `T` goes, a date with no zone as local time, the RFC 2822 form and a month by name. See spec/js.md.

use crate::js_date::{fields_to_time, DateZone, MAX_TIME_MS};

const MONTHS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];

/// The words V8 knows beside the month names: the zone names, in hours, and am and pm as the hours they add.
const ZONES: [(&str, i64); 12] = [
    ("ut", 0),
    ("utc", 0),
    ("z", 0),
    ("gmt", 0),
    ("cdt", -5),
    ("cst", -6),
    ("edt", -4),
    ("est", -5),
    ("mdt", -6),
    ("mst", -7),
    ("pdt", -7),
    ("pst", -8),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Word {
    Month(i64),
    AmPm(i64),
    Zone(i64),
    TimeSeparator,
    Garbage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Token {
    Number { value: i64, length: usize },
    Symbol(char),
    Space,
    Keyword { word: Word, length: usize },
    Unknown,
    End,
}

struct Scanner {
    chars: Vec<char>,
    at: usize,
    peeked: Option<Token>,
}

impl Scanner {
    fn scan(&mut self) -> Token {
        let Some(&c) = self.chars.get(self.at) else {
            return Token::End;
        };
        if c.is_ascii_digit() {
            let start = self.at;
            while self.chars.get(self.at) == Some(&'0') {
                self.at += 1;
            }
            let (mut value, mut significant) = (0i64, 0);
            while self.chars.get(self.at).is_some_and(char::is_ascii_digit) {
                if significant < 9 {
                    value = value * 10 + i64::from(self.chars[self.at].to_digit(10).expect("a digit"));
                }
                significant += 1;
                self.at += 1;
            }
            return Token::Number { value, length: self.at - start };
        }
        if matches!(c, ':' | '-' | '+' | '.' | ')') {
            self.at += 1;
            return Token::Symbol(c);
        }
        if c >= 'A' && !is_space(c) {
            let mut prefix = String::new();
            let mut length = 0;
            while self.chars.get(self.at).is_some_and(|c| *c >= 'A' && !is_space(*c)) {
                if length < 3 {
                    prefix.push(self.chars[self.at].to_ascii_lowercase());
                }
                length += 1;
                self.at += 1;
            }
            return Token::Keyword { word: keyword(&prefix, length), length };
        }
        if is_space(c) {
            while self.chars.get(self.at).is_some_and(|c| is_space(*c)) {
                self.at += 1;
            }
            return Token::Space;
        }
        if c == '(' {
            let mut depth = 0;
            while let Some(&c) = self.chars.get(self.at) {
                match c {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                self.at += 1;
                if depth == 0 {
                    break;
                }
            }
            return Token::Unknown;
        }
        self.at += 1;
        Token::Unknown
    }

    fn next(&mut self) -> Token {
        self.peeked.take().unwrap_or_else(|| self.scan())
    }

    fn peek(&mut self) -> Token {
        if self.peeked.is_none() {
            self.peeked = Some(self.scan());
        }
        self.peeked.expect("just scanned")
    }

    fn skip_symbol(&mut self, c: char) -> bool {
        if self.peek() == Token::Symbol(c) {
            self.next();
            true
        } else {
            false
        }
    }
}

fn is_space(c: char) -> bool {
    crate::js::is_js_whitespace(c)
}

/// A word of the date text: a month by its first three letters (whatever follows), another keyword only when it is no longer
/// than three letters.
fn keyword(prefix: &str, length: usize) -> Word {
    if let Some(index) = MONTHS.iter().position(|month| *month == prefix) {
        return Word::Month(index as i64 + 1);
    }
    if length > 3 {
        return Word::Garbage;
    }
    match prefix {
        "am" => Word::AmPm(0),
        "pm" => Word::AmPm(12),
        "t" => Word::TimeSeparator,
        _ => ZONES.iter().find(|(name, _)| *name == prefix).map_or(Word::Garbage, |(_, hours)| Word::Zone(*hours)),
    }
}

/// The day: up to three numbers, and a month named.
#[derive(Default)]
struct Day {
    comps: [i64; 3],
    index: usize,
    named_month: Option<i64>,
    iso: bool,
}

impl Day {
    fn add(&mut self, n: i64) -> bool {
        if self.index < 3 {
            self.comps[self.index] = n;
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn write(&mut self) -> Option<(i64, i64, i64)> {
        if self.index < 1 {
            return None;
        }
        while self.index < 3 {
            self.comps[self.index] = 1;
            self.index += 1;
        }
        let is_day = |x: i64| (1..=31).contains(&x);
        let c = self.comps;
        let (mut year, month, day) = match self.named_month {
            None => {
                if self.iso || !is_day(c[0]) {
                    (c[0], c[1], c[2])
                } else {
                    // Month, day, then year, which is 1 where only two numbers were given.
                    (c[2], c[0], c[1])
                }
            }
            Some(month) => {
                if !is_day(c[0]) {
                    (c[0], month, c[1])
                } else {
                    (c[1], month, c[0])
                }
            }
        };
        if !self.iso {
            if (0..=49).contains(&year) {
                year += 2000;
            } else if (50..=99).contains(&year) {
                year += 1900;
            }
        }
        ((1..=12).contains(&month) && is_day(day)).then_some((year, month, day))
    }
}

/// The time of day: hour, minute, second and millisecond, in that order, and the hours am or pm adds.
#[derive(Default)]
struct Time {
    comps: [i64; 4],
    index: usize,
    hour_offset: Option<i64>,
}

impl Time {
    fn is_empty(&self) -> bool {
        self.index == 0
    }

    fn add(&mut self, n: i64) -> bool {
        if self.index < 4 {
            self.comps[self.index] = n;
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn add_final(&mut self, n: i64) -> bool {
        if !self.add(n) {
            return false;
        }
        while self.index < 4 {
            self.comps[self.index] = 0;
            self.index += 1;
        }
        true
    }

    fn is_expecting(&self, n: i64) -> bool {
        (self.index == 1 && (0..=59).contains(&n))
            || (self.index == 2 && (0..=59).contains(&n))
            || (self.index == 3 && (0..=999).contains(&n))
    }

    fn write(&mut self) -> Option<[i64; 4]> {
        while self.index < 4 {
            self.comps[self.index] = 0;
            self.index += 1;
        }
        let [mut hour, minute, second, ms] = self.comps;
        if let Some(offset) = self.hour_offset {
            if !(1..=12).contains(&hour) {
                return None;
            }
            hour = hour % 12 + offset;
        }
        let ok = (0..=23).contains(&hour) && (0..=59).contains(&minute) && (0..=59).contains(&second) && (0..=999).contains(&ms);
        if !ok && !(hour == 24 && minute == 0 && second == 0 && ms == 0) {
            return None;
        }
        Some([hour, minute, second, ms])
    }
}

/// The zone: a sign, an hour and a minute, or none, which is local time.
#[derive(Default)]
struct Zone {
    sign: Option<i64>,
    hour: Option<i64>,
    minute: Option<i64>,
}

impl Zone {
    fn set(&mut self, hours: i64) {
        self.sign = Some(if hours < 0 { -1 } else { 1 });
        self.hour = Some(hours.abs());
        self.minute = Some(0);
    }

    fn is_utc(&self) -> bool {
        self.hour == Some(0) && self.minute == Some(0)
    }

    fn is_expecting(&self, n: i64) -> bool {
        self.hour.is_some() && self.minute.is_none() && (0..=59).contains(&n)
    }

    fn is_empty(&self) -> bool {
        self.hour.is_none()
    }

    /// The offset in seconds, or none for local time.
    fn write(&self) -> Option<i64> {
        let sign = self.sign?;
        Some(sign * (self.hour.unwrap_or(0) * 3600 + self.minute.unwrap_or(0) * 60))
    }
}

/// The milliseconds a fraction's digits name: the first three, however many were written.
fn milliseconds(value: i64, length: usize) -> i64 {
    match length {
        1 => value * 100,
        2 => value * 10,
        3 => value,
        _ => {
            let length = length.min(9);
            value / 10i64.pow(length as u32 - 3)
        }
    }
}

/// `Date.parse(text)`: the time value in milliseconds, NaN where V8 reads no date. A text with no zone is read in this
/// computer's zone; a date alone in the ISO form is UTC.
pub fn date_parse(text: &str) -> f64 {
    parse(text).unwrap_or(f64::NAN)
}

fn parse(text: &str) -> Option<f64> {
    let mut scanner = Scanner { chars: text.chars().collect(), at: 0, peeked: None };
    let mut day = Day::default();
    let mut time = Time::default();
    let mut zone = Zone::default();
    let first = es5(&mut scanner, &mut day, &mut time, &mut zone)?;
    let mut has_read_number = day.index > 0;
    let mut token = first;
    while token != Token::End {
        match token {
            Token::Number { value: n, .. } => {
                has_read_number = true;
                if scanner.skip_symbol(':') {
                    if scanner.skip_symbol(':') {
                        if !time.is_empty() {
                            return None;
                        }
                        time.add(n);
                        time.add(0);
                    } else {
                        if !time.add(n) {
                            return None;
                        }
                        if scanner.peek() == Token::Symbol('.') {
                            scanner.next();
                        }
                    }
                } else if scanner.skip_symbol('.') && time.is_expecting(n) {
                    time.add(n);
                    let Token::Number { value, length } = scanner.peek() else {
                        return None;
                    };
                    scanner.next();
                    time.add_final(milliseconds(value, length));
                } else if zone.is_expecting(n) {
                    zone.minute = Some(n);
                } else if time.is_expecting(n) {
                    time.add_final(n);
                    let ok = matches!(
                        scanner.peek(),
                        Token::End | Token::Space | Token::Keyword { word: Word::Zone(_), length: 1 } | Token::Symbol('+' | '-')
                    );
                    if !ok {
                        return None;
                    }
                } else {
                    if !day.add(n) {
                        return None;
                    }
                    scanner.skip_symbol('-');
                }
            }
            Token::Keyword { word, .. } => match word {
                Word::AmPm(hours) if !time.is_empty() => time.hour_offset = Some(hours),
                Word::Month(month) => {
                    day.named_month = Some(month);
                    scanner.skip_symbol('-');
                }
                Word::Zone(hours) if has_read_number => zone.set(hours),
                _ => {
                    if has_read_number {
                        return None;
                    }
                    if matches!(scanner.peek(), Token::Number { .. }) {
                        return None;
                    }
                }
            },
            Token::Symbol(c @ ('+' | '-')) if zone.is_utc() || !time.is_empty() => {
                zone.sign = Some(if c == '+' { 1 } else { -1 });
                let (mut n, mut length) = (0, 0);
                if let Token::Number { value, length: digits } = scanner.peek() {
                    scanner.next();
                    n = value;
                    length = digits;
                }
                has_read_number = true;
                if scanner.peek() == Token::Symbol(':') {
                    zone.hour = Some(n);
                    zone.minute = None;
                } else if length == 1 || length == 2 {
                    zone.hour = Some(n);
                    zone.minute = Some(0);
                } else if length == 3 || length == 4 {
                    zone.hour = Some(n / 100);
                    zone.minute = Some(n % 100);
                } else {
                    return None;
                }
            }
            Token::Symbol('+' | '-' | ')') if has_read_number => return None,
            _ => {}
        }
        token = scanner.next();
    }
    let (year, month, day_of_month) = day.write()?;
    let [hour, minute, second, ms] = time.write()?;
    let offset = zone.write();
    let local = fields_to_time(year, month - 1, day_of_month, hour, minute, second, ms)?;
    let utc = match offset {
        Some(seconds) => local - seconds * 1000,
        None => {
            if (local as f64).abs() > MAX_TIME_MS + 86_400_000.0 {
                return None;
            }
            crate::js_date::local_to_utc(local, &DateZone::system())
        }
    };
    ((utc as f64).abs() <= MAX_TIME_MS).then_some(utc as f64)
}

/// The ISO format V8 reads before the legacy reader: `YYYY`, `-MM`, `-DD`, then `T`, `HH:mm`, `:ss`, `.sss` and a zone.
/// Where the text is not that, whatever of its date part was read stays in `day` and the next token to read is returned.
/// None where the text is the ISO format and is wrong.
fn es5(scanner: &mut Scanner, day: &mut Day, time: &mut Time, zone: &mut Zone) -> Option<Token> {
    let fixed = |token: Token, length: usize| matches!(token, Token::Number { length: l, .. } if l == length);
    let value = |token: Token| match token {
        Token::Number { value, .. } => value,
        _ => 0,
    };
    match scanner.peek() {
        Token::Symbol(c @ ('+' | '-')) => {
            scanner.next();
            if !fixed(scanner.peek(), 6) {
                return Some(Token::Symbol(c));
            }
            let (sign, year) = (if c == '+' { 1 } else { -1 }, value(scanner.next()));
            if sign < 0 && year == 0 {
                return Some(Token::Symbol(c));
            }
            day.add(sign * year);
        }
        token if fixed(token, 4) => {
            day.add(value(scanner.next()));
        }
        _ => return Some(scanner.next()),
    }
    if scanner.skip_symbol('-') {
        let token = scanner.peek();
        if !fixed(token, 2) || !(1..=12).contains(&value(token)) {
            return Some(scanner.next());
        }
        day.add(value(scanner.next()));
        if scanner.skip_symbol('-') {
            let token = scanner.peek();
            if !fixed(token, 2) || !(1..=31).contains(&value(token)) {
                return Some(scanner.next());
            }
            day.add(value(scanner.next()));
        }
    }
    if scanner.peek() != (Token::Keyword { word: Word::TimeSeparator, length: 1 }) {
        if scanner.peek() != Token::End {
            return Some(scanner.next());
        }
    } else {
        scanner.next();
        let token = scanner.peek();
        if !fixed(token, 2) || !(0..=24).contains(&value(token)) {
            return None;
        }
        let hour_is_24 = value(token) == 24;
        time.add(value(scanner.next()));
        if !scanner.skip_symbol(':') {
            return None;
        }
        let token = scanner.peek();
        if !fixed(token, 2) || !(0..=59).contains(&value(token)) || (hour_is_24 && value(token) > 0) {
            return None;
        }
        time.add(value(scanner.next()));
        if scanner.skip_symbol(':') {
            let token = scanner.peek();
            if !fixed(token, 2) || !(0..=59).contains(&value(token)) || (hour_is_24 && value(token) > 0) {
                return None;
            }
            time.add(value(scanner.next()));
            if scanner.skip_symbol('.') {
                let Token::Number { value: fraction, length } = scanner.peek() else {
                    return None;
                };
                if hour_is_24 && fraction > 0 {
                    return None;
                }
                scanner.next();
                time.add(milliseconds(fraction, length));
            }
        }
        match scanner.peek() {
            Token::Keyword { word: Word::Zone(_), length: 1 } => {
                scanner.next();
                zone.set(0);
            }
            Token::Symbol(c @ ('+' | '-')) => {
                scanner.next();
                zone.sign = Some(if c == '+' { 1 } else { -1 });
                if fixed(scanner.peek(), 4) {
                    let hourmin = value(scanner.next());
                    let (hour, minute) = (hourmin / 100, hourmin % 100);
                    if !(0..=23).contains(&hour) || !(0..=59).contains(&minute) {
                        return None;
                    }
                    zone.hour = Some(hour);
                    zone.minute = Some(minute);
                } else {
                    let token = scanner.peek();
                    if !fixed(token, 2) || !(0..=23).contains(&value(token)) {
                        return None;
                    }
                    zone.hour = Some(value(scanner.next()));
                    if !scanner.skip_symbol(':') {
                        return None;
                    }
                    let token = scanner.peek();
                    if !fixed(token, 2) || !(0..=59).contains(&value(token)) {
                        return None;
                    }
                    zone.minute = Some(value(scanner.next()));
                }
            }
            _ => {}
        }
        if scanner.peek() != Token::End {
            return None;
        }
    }
    if zone.is_empty() && time.is_empty() {
        zone.set(0);
    }
    day.iso = true;
    Some(Token::End)
}
