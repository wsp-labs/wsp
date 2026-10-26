// SPDX-License-Identifier: AGPL-3.0-only
//! Every fixed sentence the browser pane says, as today's app words them (`BrowserSurface.tsx`, `PreviewEmptyState.tsx`,
//! `rightPanelStore.ts`, the desktop's `guests.ts`, `protocol:words/computer.ts`), and where a guest's new window goes.

/// PREVIEW_EMPTY in `PreviewEmptyState.tsx`: the servers list while nothing has listened and nothing was opened.
pub const PREVIEW_EMPTY: &str =
    "No preview yet. Type a port above or run a dev script, and servers listening on the thread's computer show up here.";

/// PREVIEW_NO_MATCH: what is typed matches no recent and no server.
pub const PREVIEW_NO_MATCH: &str = "No address or server matches.";

/// `LEAVE_WORDS.detail` in the desktop's `guests.ts`: the ask before a page's link leaves for another app.
pub const LEAVE_DETAIL: &str = "A page in the browser tab wants to leave wsp.";

/// `LEAVE_WORDS.open`.
pub const OPEN: &str = "Open";

/// `LEAVE_WORDS.cancel`.
pub const CANCEL: &str = "Cancel";

/// `LEAVE_WORDS.message`.
pub fn leave_message(url: &str) -> String {
    format!("Open {url} in another app?")
}

/// `guestCapLine` in `rightPanelStore.ts`: how many guests hold a page, and how many of those are in other threads.
pub fn cap_line(open: usize, elsewhere: usize) -> String {
    let where_ = match elsewhere {
        0 => String::new(),
        1 => "; 1 of them is in another thread".to_owned(),
        n => format!("; {n} of them are in other threads"),
    };
    format!("{open} browser tabs have a page open, the most wsp keeps. Close one to open another{where_}.")
}

/// `noPreviewRouteLine` in `protocol:words/computer.ts`: a port whose computer gives this pane no address.
pub fn no_route_line(port: u16, computer: &str) -> String {
    format!("{computer} opens no address this pane can reach for port {port}; it answers as localhost:{port} on that computer.")
}

/// `guestWindow`'s answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowRoad {
    /// A web page, opened in a new browser tab.
    Tab,
    /// Another app's link, asked about first.
    Ask,
    /// Nowhere.
    Drop,
}

/// Schemes that open nothing outside: this computer's files, script and the browser's own pages.
const STAYS_IN: [&str; 8] = ["file", "javascript", "data", "blob", "about", "chrome", "devtools", "filesystem"];

/// `guestWindow` in the desktop's `guests.ts`: what a guest's ask for a new window becomes.
pub fn window_road(url: &str) -> WindowRoad {
    match url::Url::parse(url) {
        Ok(parsed) if matches!(parsed.scheme(), "http" | "https") => WindowRoad::Tab,
        Ok(parsed) if !STAYS_IN.contains(&parsed.scheme()) => WindowRoad::Ask,
        _ => WindowRoad::Drop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cap_line_says_how_many_of_the_guests_are_in_other_threads() {
        assert_eq!(cap_line(6, 0), "6 browser tabs have a page open, the most wsp keeps. Close one to open another.");
        assert_eq!(
            cap_line(6, 1),
            "6 browser tabs have a page open, the most wsp keeps. Close one to open another; 1 of them is in another thread."
        );
        assert_eq!(
            cap_line(6, 4),
            "6 browser tabs have a page open, the most wsp keeps. Close one to open another; 4 of them are in other threads."
        );
    }

    #[test]
    fn the_route_line_names_the_computer() {
        assert_eq!(
            no_route_line(3000, "acme-box"),
            "acme-box opens no address this pane can reach for port 3000; it answers as localhost:3000 on that computer."
        );
        assert_eq!(leave_message("slack://open"), "Open slack://open in another app?");
    }

    #[test]
    fn a_web_page_opens_a_tab_another_apps_link_asks_and_anything_else_goes_nowhere() {
        for (url, road) in [
            ("https://github.com/", WindowRoad::Tab),
            ("http://localhost:3000/a", WindowRoad::Tab),
            ("slack://open", WindowRoad::Ask),
            ("mailto:a@b.c", WindowRoad::Ask),
            ("file:///etc/passwd", WindowRoad::Drop),
            ("javascript:alert(1)", WindowRoad::Drop),
            ("about:blank", WindowRoad::Drop),
            ("not a url", WindowRoad::Drop),
        ] {
            assert_eq!(window_road(url), road, "{url}");
        }
    }
}
