// SPDX-License-Identifier: AGPL-3.0-only
//! The app's screens, each in a folder of its own.

pub mod settings;

use settings::model::At;

/// A screen the capture flag opens alone, and how far down its page starts scrolled.
pub enum Alone {
    Settings(At, f32),
}

/// `--screen settings` or `--screen settings:<page>[@<px>]`, where the page is a word `At::parse` reads and the px how
/// far down the page is scrolled, as a capture of a page taller than the window takes it.
pub fn alone(word: &str) -> Option<Alone> {
    let (word, scroll) = match word.rsplit_once('@') {
        Some((word, px)) => (word, px.parse().ok()?),
        None => (word, 0.),
    };
    match word.split_once(':') {
        None if word == "settings" => Some(Alone::Settings(At::FIRST, scroll)),
        Some(("settings", page)) => At::parse(page).map(|at| Alone::Settings(at, scroll)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use settings::model::GroupId;

    #[test]
    fn the_capture_flag_names_a_page_and_a_scroll() {
        assert!(matches!(alone("settings"), Some(Alone::Settings(At::FIRST, s)) if s == 0.));
        assert!(matches!(alone("settings:computer:here@588"), Some(Alone::Settings(At::Computer(id), s)) if id == "here" && s == 588.));
        assert!(matches!(alone("settings:general"), Some(Alone::Settings(At::Group(GroupId::General), _))));
        assert!(alone("settings:nowhere").is_none());
        assert!(alone("usage").is_none());
        assert!(alone("settings:general@x").is_none());
    }
}
