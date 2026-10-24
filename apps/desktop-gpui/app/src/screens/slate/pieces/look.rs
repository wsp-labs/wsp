// SPDX-License-Identifier: AGPL-3.0-only
//! The words a slate may say and how each looks, after `apps/web/src/slate/pieces/look.ts`: tones in the app's own
//! tokens, figures through the formatter their word names (`slateFigure` in `protocol:slate/kit.ts`), and the tests a
//! value passes to read as a figure or a sentence.

use gpui_kit::{App, Hsla};
use serde_json::Value;

use crate::screens::usage::words::{fmt_cost, group_thousands, js_round};
use crate::ui::ActivePalette;
pub(crate) use crate::words::fmt_duration;
use crate::words::{fmt_tokens, round_half_up};

/// A value as text: nothing for null, its words for a string, JSON for a list or record.
pub fn str(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(num_text(n.as_f64().unwrap_or_default())),
        Value::Bool(b) => Some(b.to_string()),
        other => Some(other.to_string()),
    }
}

/// A finite number, else None.
pub fn num(v: Option<&Value>) -> Option<f64> {
    v?.as_f64().filter(|n| n.is_finite())
}

/// A number as JavaScript's `String(n)` writes it: no trailing `.0`.
pub fn num_text(n: f64) -> String {
    if n == 0. {
        "0".to_owned()
    } else if n.fract() == 0. && n.abs() < 1e21 {
        // JavaScript writes the shortest digits that read back, then zeros: 1.2345678901234567e19 is ...567000.
        let shortest = format!("{:e}", n.abs());
        let (digits, exp) = shortest.split_once('e').expect("an exponent");
        let digits = digits.replace('.', "");
        let zeros = exp.parse::<usize>().expect("a whole number's exponent") + 1 - digits.len();
        format!("{}{digits}{}", if n < 0. { "-" } else { "" }, "0".repeat(zeros))
    } else {
        format!("{n}")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Default,
    Muted,
    Good,
    Warning,
    Bad,
    Info,
    Accent,
}

impl Tone {
    /// The tone a value names; anything else is the default.
    pub fn of(v: Option<&Value>) -> Tone {
        match v.and_then(Value::as_str) {
            Some("muted") => Tone::Muted,
            Some("good") => Tone::Good,
            Some("warning") => Tone::Warning,
            Some("bad") => Tone::Bad,
            Some("info") => Tone::Info,
            Some("accent") => Tone::Accent,
            _ => Tone::Default,
        }
    }

    /// `TONE_INK`: the tone on a word or a figure.
    pub fn ink(self, cx: &App) -> Hsla {
        let p = cx.palette();
        match self {
            Tone::Default => p.foreground,
            Tone::Muted => p.muted_foreground,
            Tone::Good => p.success,
            Tone::Warning => p.warning,
            Tone::Bad => p.error_foreground,
            Tone::Info => p.status_input,
            Tone::Accent => p.slate_accent,
        }
    }

    /// `TONE_FILL`: the tone on a meter's fill; the default is the foreground at 55%.
    pub fn fill(self, cx: &App) -> Hsla {
        match self {
            Tone::Default => cx.palette().foreground.opacity(0.55),
            other => other.ink(cx),
        }
    }
}

/// A magnitude to `places` as Intl rounds it: half up from the shortest decimal that reads back as the double.
fn intl_fixed(n: f64, places: usize) -> String {
    let shortest = format!("{}", n.abs());
    let (int, frac) = shortest.split_once('.').unwrap_or((&shortest, ""));
    round_half_up(int, frac, places)
}

/// `toLocaleString("en-US")` to at most `places`: grouped, trailing zeros dropped, a negative signed even where it
/// rounds to zero.
pub fn en_us(n: f64, places: usize) -> String {
    let fixed = intl_fixed(n, places);
    let (int, frac) = fixed.split_once('.').unwrap_or((&fixed, ""));
    let frac = frac.trim_end_matches('0');
    let sign = if n.is_sign_negative() { "-" } else { "" };
    if frac.is_empty() {
        format!("{sign}{}", group_thousands(int))
    } else {
        format!("{sign}{}.{frac}", group_thousands(int))
    }
}

/// `plain` in `slateFigure`: at most two places.
pub fn plain(n: f64) -> String {
    en_us(n, 2)
}

/// `fmtInr`: rupees in lakh grouping with the sign, paise only where the value has them.
pub fn fmt_inr(inr: f64) -> String {
    let paise = (js_round(inr * 100.) as i64) % 100 != 0;
    let fixed = intl_fixed(inr, if paise { 2 } else { 0 });
    let (int, frac) = fixed.split_once('.').unwrap_or((&fixed, ""));
    let grouped = if int.len() <= 3 {
        int.to_owned()
    } else {
        let (head, tail) = int.split_at(int.len() - 3);
        let mut parts: Vec<&str> = Vec::new();
        let mut rest = head;
        while rest.len() > 2 {
            let (a, b) = rest.split_at(rest.len() - 2);
            parts.insert(0, b);
            rest = a;
        }
        if !rest.is_empty() {
            parts.insert(0, rest);
        }
        format!("{},{tail}", parts.join(","))
    };
    let sign = if inr < 0. { "-" } else { "" };
    if frac.is_empty() {
        format!("{sign}\u{20b9}{grouped}")
    } else {
        format!("{sign}\u{20b9}{grouped}.{frac}")
    }
}

/// `slateFigure`: a number through the formatter its word names; a word this build does not know reads plain.
pub fn slate_figure(value: f64, format: &str, max: Option<f64>) -> String {
    match format {
        "percent" => format!("{}%", num_text(js_round(if let Some(m) = max.filter(|m| *m > 0.) { value / m * 100. } else { value }))),
        "fraction" => match max {
            Some(m) => format!("{}/{}", plain(value), plain(m)),
            None => plain(value),
        },
        "tokens" => fmt_tokens(value),
        "bytes" if value < 1024. => format!("{} B", num_text(value)),
        "bytes" => crate::ui::fmt_bytes(value as u64),
        "usd" => fmt_cost(value),
        "inr" => fmt_inr(value),
        "duration" => fmt_duration(value),
        "integer" => {
            let r = js_round(value);
            let s = group_thousands(&num_text(r.abs()));
            if r.is_sign_negative() {
                format!("-{s}")
            } else {
                s
            }
        }
        "none" => String::new(),
        _ => plain(value),
    }
}

/// `figure`: a finite number through its formatter, anything else as its text.
pub fn figure(value: Option<&Value>, format: Option<&Value>) -> Option<String> {
    match num(value) {
        Some(n) => Some(slate_figure(n, format.and_then(Value::as_str).unwrap_or("plain"), None)),
        None => str(value),
    }
}

/// `isFigure`: a number, or text that is one with a unit or a rate after it.
pub fn is_figure(v: Option<&Value>) -> bool {
    match v {
        Some(Value::Number(_)) => true,
        Some(Value::String(s)) => {
            let s = s.trim();
            let s = s.strip_prefix(['-', '+']).unwrap_or(s);
            let s = s.strip_prefix(['$', '\u{20ac}', '\u{a3}', '\u{20b9}', '\u{a5}']).unwrap_or(s);
            let digits = s.find(|c: char| !(c.is_ascii_digit() || c == ',' || c == '.')).unwrap_or(s.len());
            let (num, rest) = s.split_at(digits);
            if !num.starts_with(|c: char| c.is_ascii_digit()) || num.matches('.').count() > 1 || num.ends_with('.') {
                return false;
            }
            if num.contains('.') && num.split('.').next().is_some_and(|i| i.contains(',') && false) {
                return false;
            }
            let rest = rest.strip_prefix(' ').unwrap_or(rest);
            if rest.is_empty() || rest == "%" {
                return true;
            }
            let (unit, per) = rest.split_once('/').map_or((rest, None), |(u, p)| (u, Some(p)));
            (1..=5).contains(&unit.len())
                && unit.chars().all(|c| c.is_ascii_alphabetic())
                && per.is_none_or(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphabetic()))
        }
        _ => false,
    }
}

/// `isSentence`: three plain words or more, so mono on it draws in the normal face.
pub fn is_sentence(v: Option<&Value>) -> bool {
    let Some(Value::String(s)) = v else { return false };
    s.split_whitespace()
        .filter(|w| {
            let w = w.strip_suffix([',', '.', ':', ';', '!', '?']).unwrap_or(w);
            let mut c = w.chars();
            c.next().is_some_and(|f| f.is_ascii_alphabetic()) && c.all(|x| x.is_ascii_lowercase())
        })
        .count()
        >= 3
}

#[cfg(test)]
mod tests {
    use gpui_kit::TestAppContext;
    use serde_json::json;

    use super::*;

    #[test]
    fn a_whole_number_past_an_i64_reads_as_javascript_writes_it() {
        assert_eq!(num_text(1e20), "100000000000000000000");
        assert_eq!(num_text(-12345678901234567890.), "-12345678901234567000");
        assert_eq!(num_text(-0.), "0");
        assert_eq!(num_text(12.), "12");
        assert_eq!(num_text(0.5), "0.5");
    }

    #[gpui_kit::test]
    fn the_accent_is_the_themes_slate_accent(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
            let hex = |c: Hsla| {
                let c = c.to_rgb();
                [c.r, c.g, c.b].map(|v| (v * 255.).round() as u8)
            };
            // The hero field in tungsten, moss and linen; the primary in the rest.
            for (theme, want) in [
                ("paper", [0x1b, 0x4e, 0xd8]),
                ("tungsten", [0xfe, 0x9a, 0x00]),
                ("moss", [0x9a, 0xe6, 0x00]),
                ("linen", [0x00, 0x78, 0x6f]),
            ] {
                crate::ui::theme::set_theme(theme, cx);
                assert_eq!(hex(Tone::Accent.ink(cx)), want, "{theme}");
            }
        });
    }

    #[test]
    fn figures_read_as_the_web_writes_them() {
        assert_eq!(slate_figure(4.64, "usd", None), "$4.64");
        assert_eq!(slate_figure(62., "percent", None), "62%");
        assert_eq!(slate_figure(3., "fraction", Some(4.)), "3/4");
        assert_eq!(slate_figure(1_966_000., "bytes", None), "2 MB");
        assert_eq!(slate_figure(130_000., "bytes", None), "127 KB");
        assert_eq!(slate_figure(1_610_612_736., "bytes", None), "1.5 GB");
        assert_eq!(slate_figure(1.5, "bytes", None), "1.5 B");
        assert_eq!(slate_figure(-5., "bytes", None), "-5 B");
        assert_eq!(slate_figure(1234.5678, "plain", None), "1,234.57");
        assert_eq!(slate_figure(420., "integer", None), "420");
        assert_eq!(slate_figure(144_740., "inr", None), "\u{20b9}1,44,740");
        assert_eq!(slate_figure(64_000., "duration", None), "1m 4s");
    }

    #[test]
    fn figures_round_and_sign_as_intl_does() {
        assert_eq!(slate_figure(1.005, "plain", None), "1.01");
        assert_eq!(slate_figure(2.675, "plain", None), "2.68");
        assert_eq!(slate_figure(-0.001, "plain", None), "-0");
        assert_eq!(slate_figure(-1234.5, "plain", None), "-1,234.5");
        assert_eq!(slate_figure(-0.3, "integer", None), "-0");
        assert_eq!(slate_figure(-1234.5, "integer", None), "-1,234");
        assert_eq!(slate_figure(-0.3, "percent", None), "0%");
        assert_eq!(slate_figure(62.5, "percent", None), "63%");
        assert_eq!(slate_figure(-1234.5, "inr", None), "-\u{20b9}1,234.50");
        assert_eq!(slate_figure(99_999.999, "inr", None), "\u{20b9}1,00,000");
    }

    #[test]
    fn durations_read_as_the_short_style() {
        let cases = [
            (0., "1ms"),
            (999.6, "1000ms"),
            (4000., "4.0s"),
            (9960., "10s"),
            (12_345., "12s"),
            (59_999., "60s"),
            (60_000., "1m"),
            (3_600_000., "60m"),
            (3_661_000., "61m 1s"),
            (-5., "0ms"),
        ];
        for (ms, want) in cases {
            assert_eq!(fmt_duration(ms), want, "{ms} ms");
        }
    }

    #[test]
    fn figures_and_sentences_are_told_apart() {
        assert!(is_figure(Some(&json!("12s"))));
        assert!(is_figure(Some(&json!("$4.20"))));
        assert!(is_figure(Some(&json!("3 MB/s"))));
        assert!(!is_figure(Some(&json!("1m 4s"))));
        assert!(!is_figure(Some(&json!("main"))));
        assert!(is_sentence(Some(&json!("the build is green"))));
        assert!(!is_sentence(Some(&json!("src/main.rs"))));
    }
}
