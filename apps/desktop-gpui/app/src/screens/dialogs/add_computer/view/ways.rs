// SPDX-License-Identifier: AGPL-3.0-only
//! What each way of signing in is called and what it does, as the rows and the picker say it.

use super::super::words;
use crate::ui::catalog;

/// `signInWayDoes`: what one way of signing in does, `field` where the key is pasted under the row.
pub fn way_does(agent: &str, name: &str, way: &str, here: &str, field: bool) -> String {
    match way {
        "token" => {
            let mint = catalog::sign_in(agent).and_then(|s| s.mint).map_or_else(|| format!("{name}'s own command"), str::to_owned);
            format!("{mint} runs on {here} and asks your browser once")
        }
        "key" if field => {
            format!("paste your {}; wsp saves it to ~/.wsp/.env without showing it", catalog::key_env(agent).unwrap_or("key"))
        }
        "key" => format!(
            "put {} in ~/.wsp/.env on {here} first, and wsp reads it from there without showing it",
            catalog::key_env(agent).unwrap_or("the key")
        ),
        _ => format!("{name}'s own sign-in runs there"),
    }
}

/// `wayNote`: what a row picked to take a token or a key says it does, as its failed row's fix opens each way with.
pub(super) fn way_note(agent: &str, name: &str, way: &str, here: &str) -> String {
    let does = way_does(agent, name, way, here, false);
    // The token's sentence opens with the command that makes it, which keeps its own case.
    if way == "token" {
        format!("{does}.")
    } else {
        format!("{}.", words::capitalised(&does))
    }
}

/// `signInWaysFix`: a sentence per way, each opening with its button's name and saying what pressing it does.
pub(super) fn ways_fix(agent: &str, name: &str, ways: &[&str], here: &str, place: &str) -> String {
    ways.iter().map(|w| format!("{}: {}.", way_label(w, place), way_does(agent, name, w, here, true))).collect::<Vec<_>>().join(" ")
}

/// `signInWayLabel`: each way's name, on its button and at the head of its sentence.
pub(super) fn way_label(way: &str, place: &str) -> String {
    match way {
        "token" => "Make a token".into(),
        "key" => "Use an API key".into(),
        _ => words::sign_in_on(place),
    }
}

/// `signInWords`: each way of signing in as the picker says it.
pub(super) fn sign_in_word(way: &str, name: &str, kind: Option<&str>) -> String {
    match way {
        "vault" => "Copy the key".into(),
        "token" => "Make a token".into(),
        "key" => "Use an API key".into(),
        _ => {
            let on = if name.is_empty() { "Sign in there".to_owned() } else { format!("Sign in on {name}") };
            if kind == Some("device") {
                format!("{on} with a code")
            } else {
                on
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{way_note, ways_fix};

    #[test]
    fn each_way_says_what_it_does_in_the_catalogs_words() {
        assert_eq!(way_note("claude", "Claude Code", "token", "Studio"), "claude setup-token runs on Studio and asks your browser once.");
        assert_eq!(
            way_note("claude", "Claude Code", "key", "Studio"),
            "Put ANTHROPIC_API_KEY in ~/.wsp/.env on Studio first, and wsp reads it from there without showing it."
        );
        assert_eq!(
            ways_fix("claude", "Claude Code", &["key", "machine"], "Studio", "lab-box"),
            "Use an API key: paste your ANTHROPIC_API_KEY; wsp saves it to ~/.wsp/.env without showing it. Sign in on lab-box: Claude Code's own sign-in runs there."
        );
    }
}
