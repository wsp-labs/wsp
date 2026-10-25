// SPDX-License-Identifier: AGPL-3.0-only
//! The two text rules of the TS host that Rust computes otherwise and every adapter shares: `String.prototype.localeCompare`
//! (rule 8.8 of the host spec) and `JSON.stringify` on a whole value (rules 4.25 to 4.28 and 8.25).

use std::cmp::Ordering;
use std::sync::OnceLock;

use icu_collator::options::CollatorOptions;
use icu_collator::{Collator, CollatorBorrowed};
use icu_locale_core::subtags::{language, script};
use icu_locale_core::Locale;
use serde_json::Value;

use crate::types::JsNumber;

/// The locale node's `localeCompare` sorts in where no call names one: ICU's default, which node reads from `LC_ALL`,
/// else `LC_MESSAGES`, else `LANG` (`LC_COLLATE` is never read). The first of the three that is set is the one, an empty
/// one included, which is the root locale `und`. `C`, `C.UTF-8`, `POSIX` and no variable at all give `en-US`;
/// `sv_SE.UTF-8` is `sv-SE`, its codeset and `@modifier` dropped; a value that is no locale gives `en-US`; a legacy
/// language code is its current one (`iw_IL` is `he-IL`).
pub fn locale_from_env(var: impl Fn(&str) -> Option<String>) -> String {
    let Some(said) = ["LC_ALL", "LC_MESSAGES", "LANG"].into_iter().find_map(var) else { return "en-US".to_string() };
    let name = said.split(['.', '@']).next().unwrap_or_default();
    match name {
        "" => "und".to_string(),
        "C" | "POSIX" => "en-US".to_string(),
        _ => name.replace('_', "-").parse::<Locale>().map_or_else(|_| "en-US".to_string(), |locale| current_language(locale).to_string()),
    }
}

/// ICU's legacy language aliases, which node resolves before it collates: `iw` is `he`, `in` is `id`, `ji` is `yi`, `mo` is `ro`,
/// `tl` is `fil`, `cmn` is `zh`, and `sh` is `sr-Latn`.
fn current_language(mut locale: Locale) -> Locale {
    let current = match locale.id.language.as_str() {
        "iw" => language!("he"),
        "in" => language!("id"),
        "ji" => language!("yi"),
        "mo" => language!("ro"),
        "tl" => language!("fil"),
        "cmn" => language!("zh"),
        "sh" => {
            locale.id.script.get_or_insert(script!("Latn"));
            language!("sr")
        }
        _ => return locale,
    };
    locale.id.language = current;
    locale
}

/// The collator `localeCompare` is in `locale`: ICU4X's, at tertiary strength with nothing ignored, the options a call
/// with no options takes. A locale with no collation of its own sorts as the root does, as ICU does.
pub fn collator(locale: &str) -> CollatorBorrowed<'static> {
    let locale: Locale = locale.parse().unwrap_or_else(|_| "en-US".parse().expect("en-US is a locale"));
    Collator::try_new((&locale).into(), CollatorOptions::default()).expect("the compiled collation data holds every locale's rules")
}

/// `a.localeCompare(b)` in the host's own locale (`locale_from_env` on this process's environment, read once), where only
/// the sign of the TS result is read: case and accent are tie-breakers below the letters, punctuation and spaces sort
/// before digits, digits before letters.
pub fn collate(a: &str, b: &str) -> Ordering {
    static HOST: OnceLock<CollatorBorrowed<'static>> = OnceLock::new();
    HOST.get_or_init(|| collator(&locale_from_env(|name| std::env::var(name).ok()))).compare(a, b)
}

/// `JSON.stringify(value)` of the JavaScript value `JSON.parse` makes of `value`'s own text: no white space; every
/// string escaped as JavaScript does (`"`, `\`, the control characters as `\b \t \n \f \r` or `\u00xx` in lower case;
/// U+2028, U+2029, DEL, `/` and every other character as themselves); every number as `String(n)` (an integral value
/// with no `.0`, `1e+21`, `-0` as `0`, the double a value past 2^53 rounds to); each object's keys in the order
/// JavaScript holds them, which is the array-index keys (`"0"`, `"7"`, not `"07"`) ascending and then the rest in the
/// order given.
pub fn json_stringify(value: &Value) -> String {
    let mut out = String::new();
    write_value(value, &mut out);
    out
}

fn write_value(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => out.push_str(&JsNumber(n.as_f64().expect("a JSON number reads as a double")).js_string()),
        Value::String(text) => out.push_str(&serde_json::to_string(text).expect("a string writes as JSON")),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(item, out);
            }
            out.push(']');
        }
        Value::Object(map) => {
            let index = |key: &str| {
                let canonical = key == "0" || (!key.is_empty() && !key.starts_with('0') && key.bytes().all(|b| b.is_ascii_digit()));
                canonical.then(|| key.parse::<u64>().ok().filter(|n| *n < 4_294_967_295)).flatten()
            };
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by_key(|(key, _)| index(key).map_or((1, 0), |n| (0, n)));
            out.push('{');
            for (i, (key, item)) in entries.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key).expect("a key writes as JSON"));
                out.push(':');
                write_value(item, out);
            }
            out.push('}');
        }
    }
}
