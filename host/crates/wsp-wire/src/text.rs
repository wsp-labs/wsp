// SPDX-License-Identifier: AGPL-3.0-only
//! The two text rules of the TS host that Rust computes otherwise and every adapter shares: `String.prototype.localeCompare`
//! (rule 8.8 of the host spec) and `JSON.stringify` on a whole value (rules 4.25 to 4.28 and 8.25).

use std::borrow::Cow;
use std::cmp::Ordering;
use std::sync::OnceLock;

use icu_collator::options::CollatorOptions;
use icu_collator::provider::CollationMetadataV1;
use icu_collator::Collator;
use icu_locale_core::subtags::{language, region, script, variant};
use icu_locale_core::{LanguageIdentifier, Locale};
use icu_normalizer::{DecomposingNormalizer, DecomposingNormalizerBorrowed};
use icu_provider::{DataIdentifierBorrowed, DataLocale, DataProvider, DataRequest, DataRequestMetadata};
use serde_json::Value;

use crate::types::JsNumber;

/// ICU 78.3's collation data for every locale, exported from CLDR 48 with the Unihan radical-stroke order of Han
/// characters (`spec/text.md`, decision 3).
#[allow(dead_code)]
mod baked {
    pub struct Baked;

    include!("text/collation_data/mod.rs");
    impl_data_provider!(Baked);
}

use baked::Baked;

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

/// ICU's legacy language aliases, which node resolves before it collates: `iw` is `he`, `in` is `id`, `ji` is `yi`, `mo` is `ro`
/// and `sh` is `sr-Latn`, and `en_US_POSIX` is `en-US`. `tl` and `cmn` stay as they are: ICU has no rules for either, so both
/// sort as the root.
fn current_language(mut locale: Locale) -> Locale {
    let current = match locale.id.language.as_str() {
        "iw" => language!("he"),
        "in" => language!("id"),
        "ji" => language!("yi"),
        "mo" => language!("ro"),
        "sh" => {
            locale.id.script.get_or_insert(script!("Latn"));
            language!("sr")
        }
        "en" if locale.id.region == Some(region!("US")) && *locale.id.variants == [variant!("posix")] => {
            locale.id.variants.clear();
            return locale;
        }
        _ => return locale,
    };
    locale.id.language = current;
    locale
}

/// A collator for one locale over the baked data. The decomposer is held for Lithuanian only, see `apart_from_marks`.
pub struct TextCollator {
    collator: Collator,
    decomposer: Option<DecomposingNormalizer>,
}

impl TextCollator {
    pub fn compare(&self, a: &str, b: &str) -> Ordering {
        let collator = self.collator.as_borrowed();
        match &self.decomposer {
            Some(decomposer) => {
                collator.compare(&apart_from_marks(decomposer.as_borrowed(), a), &apart_from_marks(decomposer.as_borrowed(), b))
            }
            None => collator.compare(a, b),
        }
    }
}

/// The collator `localeCompare` is in `locale`: ICU4X's, at tertiary strength with nothing ignored, the options a call
/// with no options takes. A locale with no collation of its own sorts as the root does, as ICU does.
pub fn collator(locale: &str) -> TextCollator {
    let locale: Locale = locale.parse().unwrap_or_else(|_| "en-US".parse().expect("en-US is a locale"));
    let rules = rules_locale(&locale);
    TextCollator {
        collator: Collator::try_new_unstable(&Baked, (&rules).into(), CollatorOptions::default())
            .expect("the baked collation data holds every locale's rules"),
        decomposer: (rules.id.language == language!("lt"))
            .then(|| DecomposingNormalizer::try_new_nfd_unstable(&Baked).expect("the baked data holds the decompositions")),
    }
}

/// Lithuanian ignores a dot above (U+0307) that sits directly before a grave, an acute or a tilde. ICU4X decomposes its
/// input first and so ignores the dot of a letter that has one (`ẇ`, `ż`) followed by such a mark; ICU does not
/// normalize, so the dot inside a precomposed letter stays, as `ẇ` against `ẇ` with U+0303 shows. A combining grapheme
/// joiner (U+034F, which collates as nothing) after such a letter keeps its dot from touching the mark.
fn apart_from_marks<'a>(decomposer: DecomposingNormalizerBorrowed<'_>, text: &'a str) -> Cow<'a, str> {
    let mut apart = String::new();
    let mut chars = text.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        let next = chars.peek().map(|(_, next)| *next);
        if c >= '\u{c0}'
            && c != '\u{307}'
            && matches!(next, Some('\u{300}' | '\u{301}' | '\u{303}'))
            && decomposer.normalize(c.encode_utf8(&mut [0; 4])).ends_with('\u{307}')
        {
            if apart.is_empty() {
                apart.push_str(&text[..at]);
            }
            apart.push(c);
            apart.push('\u{34f}');
        } else if !apart.is_empty() {
            apart.push(c);
        }
    }
    if apart.is_empty() {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(apart)
    }
}

/// The most specific locale with rules of its own that `locale` reaches: language-script-region, language-script,
/// language-region, then the language, each by its own subtags, never through the likely script ICU4X would add (`sr-ME`
/// sorts as `sr`, Cyrillic first, not as `sr-Latn`). The root (`und`) where none has rules. Three languages are left to the
/// likely script, as node leaves them: with no script of its own `zh-US` is Traditional (stroke order) and `kk-CN` is Arabic,
/// and `zh` and `yue` keep their rules under the scripts `und-Hans` and `und-Hant`. `nb` and `nn` are `no`.
fn rules_locale(locale: &Locale) -> Locale {
    let id = &locale.id;
    let language = match id.language.as_str() {
        "nb" | "nn" => language!("no"),
        _ => id.language,
    };
    if language == language!("zh") || language == language!("yue") {
        let script = id.script.filter(|s| *s == script!("Hans") || *s == script!("Hant"));
        let region = if id.script.is_none() { id.region } else { None };
        return Locale::from(LanguageIdentifier::from((language, script, region)));
    }
    if language == language!("kk") && id.script.is_none() {
        return Locale::from(LanguageIdentifier::from((language, None, id.region)));
    }
    for (script, region) in [(id.script, id.region), (id.script, None), (None, id.region), (None, None)] {
        let candidate = Locale::from(LanguageIdentifier::from((language, script, region)));
        let data = DataLocale::from(&candidate);
        let mut metadata = DataRequestMetadata::default();
        metadata.silent = true;
        let request = DataRequest { id: DataIdentifierBorrowed::for_locale(&data), metadata };
        if let Ok(found) = DataProvider::<CollationMetadataV1>::load(&Baked, request) {
            // The baked provider names a locale in its answer only when it fell back from the one asked for.
            if found.metadata.locale.is_none() {
                return candidate;
            }
        }
    }
    Locale::UNKNOWN
}

/// `a.localeCompare(b)` in the host's own locale (`locale_from_env` on this process's environment, read once), where only
/// the sign of the TS result is read: case and accent are tie-breakers below the letters, punctuation and spaces sort
/// before digits, digits before letters.
pub fn collate(a: &str, b: &str) -> Ordering {
    static HOST_LOCALE: OnceLock<String> = OnceLock::new();
    thread_local!(static HOST: TextCollator = collator(HOST_LOCALE.get_or_init(|| locale_from_env(|name| std::env::var(name).ok()))));
    HOST.with(|host| host.compare(a, b))
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
