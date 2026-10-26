// SPDX-License-Identifier: AGPL-3.0-only
//! What is behind one browser tab (`browser/tabs.ts`): the places it has been and which one it shows. A guest reports
//! each page it goes to, which the tab follows in place while the guest keeps the history itself.

use super::url::{host_of, port_and_path, Place};

pub const ZOOM_MIN: f32 = 0.5;
pub const ZOOM_MAX: f32 = 2.0;
pub const ZOOM_STEP: f32 = 0.1;

/// `BrowserTabState`.
#[derive(Clone, Debug, PartialEq)]
pub struct Tab {
    /// Places in visit order; none is the servers list.
    pub entries: Vec<Option<Place>>,
    pub index: usize,
    /// Counts the places typed or picked, which a guest loads; the guest's own moves leave it.
    pub loads: u64,
    /// The guest page's own title, empty until it says one.
    pub title: String,
    /// Bumped to load the page again.
    pub reload_nonce: u64,
    pub zoom: f32,
}

impl Tab {
    /// `freshTab`: a tab starts on the servers list so back has somewhere to go.
    pub fn new(address: Option<Place>) -> Self {
        let entries = match address {
            None => vec![None],
            Some(place) => vec![None, Some(place)],
        };
        Tab { index: entries.len() - 1, entries, loads: 0, title: String::new(), reload_nonce: 0, zoom: 1.0 }
    }

    /// `currentPlace`.
    pub fn current(&self) -> Option<&Place> {
        self.entries.get(self.index).and_then(Option::as_ref)
    }

    /// A new entry after the current one, dropping any forward ones; true when the guest has a load to do, false for
    /// the place it is already on.
    pub fn navigate(&mut self, address: Option<Place>) -> bool {
        if self.current() == address.as_ref() {
            return false;
        }
        self.entries.truncate(self.index + 1);
        self.entries.push(address);
        self.index = self.entries.len() - 1;
        self.loads += 1;
        true
    }

    /// Where the guest went by itself: the current entry becomes it, with no load asked.
    pub fn follow(&mut self, place: Place) {
        if self.current() != Some(&place) {
            self.entries[self.index] = Some(place);
        }
    }

    pub fn titled(&mut self, title: &str) {
        if self.title != title {
            self.title = title.to_owned();
        }
    }

    pub fn back(&mut self) {
        self.index = self.index.saturating_sub(1);
    }

    pub fn forward(&mut self) {
        if self.index + 1 < self.entries.len() {
            self.index += 1;
        }
    }

    pub fn reload(&mut self) {
        self.reload_nonce += 1;
    }

    /// Rounded to hundredths and clamped; true when the zoom changed.
    pub fn set_zoom(&mut self, zoom: f32) -> bool {
        let clamped = ((zoom * 100.0).round() / 100.0).clamp(ZOOM_MIN, ZOOM_MAX);
        if clamped == self.zoom {
            return false;
        }
        self.zoom = clamped;
        true
    }
}

/// `previewTabSnapshots`' title: a site's own title else its host; a port as the process behind it when known and the
/// decoded port and path, never the page's title. None on the servers list, which the strip names "Browser".
pub fn tab_title(tab: &Tab, process_of_port: impl Fn(u16) -> Option<String>) -> Option<String> {
    Some(match tab.current()? {
        Place::Site(site) if tab.title.is_empty() => host_of(&site.url),
        Place::Site(_) => tab.title.clone(),
        Place::Port(at) => {
            let where_ = port_and_path(at.port, &at.path);
            match process_of_port(at.port).filter(|p| !p.is_empty()) {
                Some(process) => format!("{process} {where_}"),
                None => where_,
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::super::url::{Address, Site};
    use super::*;

    fn port(port: u16, path: &str) -> Place {
        Place::Port(Address { port, path: path.into() })
    }

    fn site(url: &str) -> Place {
        Place::Site(Site { url: url.into() })
    }

    #[test]
    fn a_new_path_on_the_same_port_is_a_new_entry_and_the_same_address_again_is_not() {
        let mut tab = Tab::new(Some(port(3000, "/")));
        assert!(tab.navigate(Some(port(3000, "/about?x=1"))));
        assert!(!tab.navigate(Some(port(3000, "/about?x=1"))));
        assert_eq!(tab.entries, [None, Some(port(3000, "/")), Some(port(3000, "/about?x=1"))]);
        assert_eq!(tab.current(), Some(&port(3000, "/about?x=1")));
        assert_eq!(tab.loads, 1);
        tab.back();
        assert_eq!(tab.current(), Some(&port(3000, "/")));
    }

    #[test]
    fn a_fresh_tab_starts_on_the_servers_list() {
        let empty = Tab::new(None);
        assert_eq!((empty.entries.len(), empty.index, empty.current()), (1, 0, None));
        let mut tab = Tab::new(Some(port(3000, "/")));
        assert_eq!((tab.index, tab.zoom, tab.loads), (1, 1.0, 0));
        tab.back();
        assert_eq!(tab.current(), None);
        tab.back();
        assert_eq!(tab.index, 0);
    }

    #[test]
    fn navigating_after_going_back_drops_the_forward_entries() {
        let mut tab = Tab::new(Some(port(3000, "/a")));
        tab.navigate(Some(port(3000, "/b")));
        tab.back();
        tab.forward();
        assert_eq!(tab.current(), Some(&port(3000, "/b")));
        tab.forward();
        assert_eq!(tab.index, 2);
        tab.back();
        tab.navigate(Some(site("https://github.com/")));
        assert_eq!(tab.entries, [None, Some(port(3000, "/a")), Some(site("https://github.com/"))]);
    }

    #[test]
    fn following_the_guest_replaces_the_current_entry_with_no_load() {
        let mut tab = Tab::new(Some(port(3000, "/")));
        tab.follow(port(3000, "/next"));
        assert_eq!(tab.entries, [None, Some(port(3000, "/next"))]);
        assert_eq!(tab.loads, 0);
    }

    #[test]
    fn zoom_is_rounded_to_hundredths_and_clamped() {
        let mut tab = Tab::new(None);
        assert!(tab.set_zoom(1.0 + ZOOM_STEP));
        assert_eq!(tab.zoom, 1.1);
        assert!(!tab.set_zoom(1.104));
        assert!(tab.set_zoom(5.0));
        assert_eq!(tab.zoom, ZOOM_MAX);
        assert!(tab.set_zoom(0.1));
        assert_eq!(tab.zoom, ZOOM_MIN);
        tab.reload();
        assert_eq!(tab.reload_nonce, 1);
    }

    #[test]
    fn the_strip_names_a_port_by_its_process_and_path_and_the_root_as_the_port_alone() {
        let node = |p: u16| (p == 5173).then(|| "node".to_owned());
        assert_eq!(tab_title(&Tab::new(Some(port(5173, "/"))), node).as_deref(), Some("node :5173"));
        assert_eq!(tab_title(&Tab::new(Some(port(3000, "/about?x=1"))), node).as_deref(), Some(":3000/about?x=1"));
        let mut titled = Tab::new(Some(port(3000, "/caf%C3%A9")));
        titled.titled("Café");
        assert_eq!(tab_title(&titled, node).as_deref(), Some(":3000/café"));
        assert_eq!(tab_title(&Tab::new(None), node), None);
    }

    #[test]
    fn the_strip_names_a_site_by_its_title_else_its_host() {
        let mut tab = Tab::new(Some(site("https://github.com/acme")));
        assert_eq!(tab_title(&tab, |_| None).as_deref(), Some("github.com"));
        tab.titled("acme");
        assert_eq!(tab_title(&tab, |_| None).as_deref(), Some("acme"));
    }
}
