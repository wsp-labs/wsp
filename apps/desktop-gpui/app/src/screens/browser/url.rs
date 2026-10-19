// SPDX-License-Identifier: AGPL-3.0-only
//! Address arithmetic for the browser pane (`browser/url.ts`): a tab is a port of the thread's computer and a path, or
//! any web page; the frame dials the minted route with the path before its query, and the bar shows the loopback form.

use std::fmt::Write as _;

use url::{form_urlencoded, Url};

/// `Address`: a port and the path, with its query, framed on it; "/" is the root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Address {
    pub port: u16,
    pub path: String,
}

/// `Site`: a page on any host but a loopback one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Site {
    pub url: String,
}

/// `Place`: where a tab is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    Port(Address),
    Site(Site),
}

pub const SEARCH_URL: &str = "https://www.google.com/search?q=";

const ROOT: &str = "/";
const TOKEN_PARAM: &str = "pt_token";
const LOOPBACK_HOSTS: [&str; 5] = ["localhost", "127.0.0.1", "0.0.0.0", "::1", "[::1]"];

fn after_port(path: &str) -> &str {
    if path == ROOT {
        ""
    } else {
        path
    }
}

/// `loopbackUrl`: "http://localhost:3000" or "http://localhost:3000/about?x=1"; recents key off this, so it stays
/// encoded.
pub fn loopback_url(port: u16, path: &str) -> String {
    format!("http://localhost:{port}{}", after_port(path))
}

/// `loopbackAddress`: "localhost:3000" or "localhost:3000/about?x=1", what the bar shows.
pub fn loopback_address(port: u16, path: &str) -> String {
    format!("localhost{}", port_and_path(port, path))
}

/// `portAndPath`: ":3000" or ":3000/about?x=1", how a tab names where it is, decoded for reading.
pub fn port_and_path(port: u16, path: &str) -> String {
    let readable = decode_uri(path).unwrap_or_else(|| path.to_owned());
    format!(":{port}{}", after_port(&readable))
}

/// `parseAddress`: the port and path a typed address means ("3000", ":3000", "3000/about", "localhost:3000/about?x=1"
/// or a loopback URL); anything on another host is not something this pane frames.
pub fn parse_address(raw: &str) -> Option<Address> {
    let text = raw.trim();
    let with_scheme = match bare_port(text) {
        Some((port, rest)) => format!("http://localhost:{port}{rest}"),
        None if has_scheme(text) => text.to_owned(),
        None => format!("http://{text}"),
    };
    let parsed = Url::parse(&with_scheme).ok()?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return None;
    }
    if !LOOPBACK_HOSTS.contains(&parsed.host_str()?.to_ascii_lowercase().as_str()) {
        return None;
    }
    let port = parsed.port_or_known_default()?;
    if port == 0 {
        return None;
    }
    Some(Address { port, path: path_of(&parsed) })
}

/// `frameSrc`: the minted route with the path before its query and the edge's token kept beside the page's own
/// parameters; a pt_token typed into the page's query is dropped so the edge only ever sees the minted one.
pub fn frame_src(route: &str, path: &str) -> Option<String> {
    let mut src = Url::parse(route).ok()?;
    let (before_hash, hash) = match path.find('#') {
        Some(at) => (&path[..at], Some(&path[at + 1..])),
        None => (path, None),
    };
    let (page_path, page_query) = match before_hash.find('?') {
        Some(at) => (&before_hash[..at], Some(&before_hash[at + 1..])),
        None => (before_hash, None),
    };
    let own = src.query().unwrap_or("").to_owned();
    let query = std::iter::once(own.as_str())
        .chain(page_query.into_iter().flat_map(|q| q.split('&')).filter(|pair| pair.split('=').next() != Some(TOKEN_PARAM)))
        .filter(|pair| !pair.is_empty())
        .collect::<Vec<_>>()
        .join("&");
    src.set_path(page_path);
    src.set_query(if query.is_empty() { None } else { Some(&query) });
    src.set_fragment(hash);
    Some(src.to_string())
}

/// `parseTarget`: a port of the thread's computer, a web address (http for a host with a port but 443, else https when
/// no scheme was typed, as a browser's bar does), or a search for the words.
pub fn parse_target(raw: &str) -> Place {
    if let Some(address) = parse_address(raw) {
        return Place::Port(address);
    }
    let text = raw.trim();
    let candidate = if starts_with_http(text) {
        Some(text.to_owned())
    } else if let Some(port) = host_port(text) {
        Some(format!("{}://{text}", if port == "443" { "https" } else { "http" }))
    } else if bare_host(text) {
        Some(format!("https://{text}"))
    } else {
        None
    };
    if let Some(url) = candidate.and_then(|c| Url::parse(&c).ok()) {
        return Place::Site(Site { url: url.to_string() });
    }
    Place::Site(Site { url: format!("{SEARCH_URL}{}", encode_uri_component(text)) })
}

/// `placeOfUrl`: on the minted route's origin a guest's url is that port with the token taken off the query, on a
/// loopback host the port itself, and anything else a web page. The route is its url and the port it stands for.
pub fn place_of_url(url: &str, route: Option<(&str, u16)>) -> Place {
    let Ok(mut parsed) = Url::parse(url) else {
        return Place::Site(Site { url: url.to_owned() });
    };
    if let Some((route_url, port)) = route {
        let same_origin = Url::parse(route_url).is_ok_and(|r| r.origin().ascii_serialization() == parsed.origin().ascii_serialization());
        if same_origin {
            // URLSearchParams.delete re-serializes the whole query as a form, so the kept pairs are re-encoded too.
            let kept = form_urlencoded::Serializer::new(String::new())
                .extend_pairs(parsed.query_pairs().filter(|(name, _)| name != TOKEN_PARAM))
                .finish();
            parsed.set_query(if kept.is_empty() { None } else { Some(&kept) });
            return Place::Port(Address { port, path: path_of(&parsed) });
        }
    }
    parse_address(url).map(Place::Port).unwrap_or_else(|| Place::Site(Site { url: url.to_owned() }))
}

/// `placeUrl`: what recents key off and a strip entry links.
pub fn place_url(place: &Place) -> String {
    match place {
        Place::Site(site) => site.url.clone(),
        Place::Port(at) => loopback_url(at.port, &at.path),
    }
}

/// `placeAddress`: what the bar shows.
pub fn place_address(place: &Place) -> String {
    match place {
        Place::Site(site) => site.url.clone(),
        Place::Port(at) => loopback_address(at.port, &at.path),
    }
}

/// `hostOf` in `browser/tabs.ts`: the url's host with its port, or the url as given when it does not parse.
pub fn host_of(url: &str) -> String {
    let Ok(parsed) = Url::parse(url) else {
        return url.to_owned();
    };
    match (parsed.host_str(), parsed.port()) {
        (Some(host), Some(port)) => format!("{host}:{port}"),
        (Some(host), None) => host.to_owned(),
        (None, _) => String::new(),
    }
}

/// pathname, search and hash as JS reads them: an empty query or fragment reads as nothing.
fn path_of(url: &Url) -> String {
    let mut out = url.path().to_owned();
    if let Some(query) = url.query().filter(|q| !q.is_empty()) {
        out.push('?');
        out.push_str(query);
    }
    if let Some(fragment) = url.fragment().filter(|f| !f.is_empty()) {
        out.push('#');
        out.push_str(fragment);
    }
    out
}

/// The `(?:[/?#].*)?$` tail of the web regexes, whose `.` stops at a line break.
fn tail_ok(rest: &str) -> bool {
    rest.is_empty() || (rest.starts_with(['/', '?', '#']) && !rest.contains(['\n', '\r', '\u{2028}', '\u{2029}']))
}

/// `/^:?(\d{1,5})([/?#].*)?$/`: the digits and what follows them.
fn bare_port(text: &str) -> Option<(&str, &str)> {
    let body = text.strip_prefix(':').unwrap_or(text);
    let digits = body.bytes().take_while(u8::is_ascii_digit).count();
    let (port, rest) = body.split_at(digits);
    ((1..=5).contains(&digits) && tail_ok(rest)).then_some((port, rest))
}

/// `/^[a-z][a-z0-9+.-]*:\/\//i`.
fn has_scheme(text: &str) -> bool {
    let scheme = text.bytes().take_while(|b| b.is_ascii_alphanumeric() || b"+.-".contains(b)).count();
    text.as_bytes().first().is_some_and(u8::is_ascii_alphabetic) && text[scheme..].starts_with("://")
}

/// `/^https?:\/\//i`.
fn starts_with_http(text: &str) -> bool {
    let prefix = |p: &str| text.get(..p.len()).is_some_and(|head| head.eq_ignore_ascii_case(p));
    prefix("http://") || prefix("https://")
}

fn host_char(b: &u8) -> bool {
    b.is_ascii_alphanumeric() || *b == b'.' || *b == b'-'
}

/// `HOST_PORT`, `/^[a-z0-9][a-z0-9.-]*:(\d{1,5})(?:[/?#].*)?$/i`: the typed port.
fn host_port(text: &str) -> Option<&str> {
    let host = text.bytes().take_while(host_char).count();
    if host == 0 || !text.as_bytes()[0].is_ascii_alphanumeric() {
        return None;
    }
    let after = text[host..].strip_prefix(':')?;
    let digits = after.bytes().take_while(u8::is_ascii_digit).count();
    ((1..=5).contains(&digits) && tail_ok(&after[digits..])).then(|| &after[..digits])
}

/// `BARE_HOST`, `/^(?:[a-z0-9-]+\.)+(?:[a-z]{2,}|\d{1,3})(?:[/?#].*)?$/i`: a dotted name ending in letters or an IPv4
/// address.
fn bare_host(text: &str) -> bool {
    let end = text.bytes().take_while(host_char).count();
    if !tail_ok(&text[end..]) {
        return false;
    }
    let labels: Vec<&str> = text[..end].split('.').collect();
    let Some((last, rest)) = labels.split_last() else {
        return false;
    };
    let tld = (last.len() >= 2 && last.bytes().all(|b| b.is_ascii_alphabetic()))
        || ((1..=3).contains(&last.len()) && last.bytes().all(|b| b.is_ascii_digit()));
    !rest.is_empty() && rest.iter().all(|label| !label.is_empty()) && tld
}

/// JS `encodeURIComponent`.
fn encode_uri_component(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

/// JS `decodeURI`: keeps the escapes of characters that would change the path's shape, and none for a malformed escape,
/// where JS throws.
fn decode_uri(text: &str) -> Option<String> {
    const RESERVED: &[u8] = b";/?:@&=+$,#";
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'%' {
            out.push(bytes[i]);
            i += 1;
            continue;
        }
        let first = escaped(bytes, i)?;
        if first < 0x80 {
            if RESERVED.contains(&first) {
                out.extend_from_slice(&bytes[i..i + 3]);
            } else {
                out.push(first);
            }
            i += 3;
            continue;
        }
        let n = first.leading_ones() as usize;
        if !(2..=4).contains(&n) {
            return None;
        }
        let mut sequence = vec![first];
        for k in 1..n {
            let next = escaped(bytes, i + 3 * k)?;
            if next & 0xC0 != 0x80 {
                return None;
            }
            sequence.push(next);
        }
        std::str::from_utf8(&sequence).ok()?;
        out.extend_from_slice(&sequence);
        i += 3 * n;
    }
    String::from_utf8(out).ok()
}

fn escaped(bytes: &[u8], at: usize) -> Option<u8> {
    let hex = |b: &u8| (*b as char).to_digit(16);
    match bytes.get(at..at + 3)? {
        [b'%', high, low] => Some((hex(high)? * 16 + hex(low)?) as u8),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn port(port: u16, path: &str) -> Place {
        Place::Port(Address { port, path: path.into() })
    }

    fn site(url: &str) -> Place {
        Place::Site(Site { url: url.into() })
    }

    #[test]
    fn a_typed_address_means_a_port_and_a_path() {
        for (raw, port, path) in [
            ("3000", 3000, "/"),
            (":3000", 3000, "/"),
            ("localhost:3000", 3000, "/"),
            ("http://localhost:3000", 3000, "/"),
            ("http://localhost:3000/", 3000, "/"),
            ("3000/about", 3000, "/about"),
            (":3000/about", 3000, "/about"),
            ("localhost:3000/about?x=1", 3000, "/about?x=1"),
            ("http://localhost:3000/about?x=1", 3000, "/about?x=1"),
            ("https://127.0.0.1:8443/a/b/?x=1&y=2#top", 8443, "/a/b/?x=1&y=2#top"),
            ("  localhost:3000/about  ", 3000, "/about"),
            ("localhost/about", 80, "/about"),
            ("https://localhost", 443, "/"),
            ("3000?x=1", 3000, "/?x=1"),
            ("localhost:3000?x=1", 3000, "/?x=1"),
            ("3000#top", 3000, "/#top"),
            ("localhost:3000/café?name=zoë", 3000, "/caf%C3%A9?name=zo%C3%AB"),
        ] {
            assert_eq!(parse_address(raw), Some(Address { port, path: path.into() }), "{raw}");
        }
    }

    #[test]
    fn another_host_a_bad_port_or_a_word_is_not_an_address_this_pane_frames() {
        for raw in ["example.com:3000/about", "ftp://localhost:21/x", "70000", ":0", "about", "3000about", ""] {
            assert_eq!(parse_address(raw), None, "{raw}");
        }
    }

    const ROUTE: &str = "https://m1-3000.preview.example/?pt_token=edge";

    fn framed(route: &str, path: &str) -> String {
        frame_src(route, path).unwrap()
    }

    #[test]
    fn the_minted_route_is_left_alone_at_the_root() {
        assert_eq!(framed(ROUTE, "/"), ROUTE);
    }

    #[test]
    fn the_path_goes_before_the_query_and_the_token_sits_beside_the_pages_own_parameters() {
        assert_eq!(framed(ROUTE, "/about?x=1"), "https://m1-3000.preview.example/about?pt_token=edge&x=1");
    }

    #[test]
    fn a_route_with_no_query_of_its_own_keeps_the_pages_query_whole() {
        assert_eq!(framed("http://localhost:8080/", "/app?x=1&y=2"), "http://localhost:8080/app?x=1&y=2");
        assert_eq!(framed("http://localhost:8080/", "/app"), "http://localhost:8080/app");
    }

    #[test]
    fn a_fragment_rides_after_the_query() {
        assert_eq!(framed(ROUTE, "/docs#install"), "https://m1-3000.preview.example/docs?pt_token=edge#install");
    }

    #[test]
    fn a_pt_token_typed_into_the_pages_query_is_dropped() {
        assert_eq!(framed(ROUTE, "/about?pt_token=evil&x=1"), "https://m1-3000.preview.example/about?pt_token=edge&x=1");
        assert_eq!(framed(ROUTE, "/about?x=1&pt_token=evil"), "https://m1-3000.preview.example/about?pt_token=edge&x=1");
        assert_eq!(framed(ROUTE, "/about?pt_token=evil"), "https://m1-3000.preview.example/about?pt_token=edge");
    }

    #[test]
    fn the_path_and_the_query_keep_their_encoded_form() {
        assert_eq!(framed(ROUTE, "/a%20b/c?q=hello%20world"), "https://m1-3000.preview.example/a%20b/c?pt_token=edge&q=hello%20world");
        assert_eq!(framed(ROUTE, "/caf%C3%A9?name=zo%C3%AB"), "https://m1-3000.preview.example/caf%C3%A9?pt_token=edge&name=zo%C3%AB");
    }

    #[test]
    fn the_edges_own_form_of_the_route_with_no_slash_before_the_token_is_taken() {
        assert_eq!(
            framed("https://abc-8000.preview.getsolari.com?pt_token=t", "/sub/index.html"),
            "https://abc-8000.preview.getsolari.com/sub/index.html?pt_token=t"
        );
    }

    #[test]
    fn the_loopback_url_and_the_bars_form_carry_no_slash_at_the_root() {
        assert_eq!(loopback_url(3000, "/"), "http://localhost:3000");
        assert_eq!(loopback_url(3000, "/about?x=1"), "http://localhost:3000/about?x=1");
        assert_eq!(loopback_address(3000, "/"), "localhost:3000");
        assert_eq!(loopback_address(3000, "/about?x=1"), "localhost:3000/about?x=1");
    }

    #[test]
    fn what_a_person_reads_is_decoded_and_what_a_browser_dials_stays_encoded() {
        assert_eq!(loopback_url(3000, "/caf%C3%A9?name=zo%C3%AB"), "http://localhost:3000/caf%C3%A9?name=zo%C3%AB");
        assert_eq!(loopback_address(3000, "/caf%C3%A9?name=zo%C3%AB"), "localhost:3000/café?name=zoë");
        assert_eq!(port_and_path(3000, "/caf%C3%A9"), ":3000/café");
        assert_eq!(loopback_address(3000, "/a%2Fb%3Fc"), "localhost:3000/a%2Fb%3Fc");
        assert_eq!(loopback_address(3000, "/%E0%A4%A"), "localhost:3000/%E0%A4%A");
    }

    #[test]
    fn the_bar_reads_a_port_a_web_address_or_a_search() {
        let search = |words: &str| site(&format!("{SEARCH_URL}{words}"));
        for (raw, place) in [
            ("3000", port(3000, "/")),
            ("localhost:5173/a", port(5173, "/a")),
            ("github.com", site("https://github.com/")),
            ("  docs.example.co.uk/guide?x=1 ", site("https://docs.example.co.uk/guide?x=1")),
            ("http://example.org/a", site("http://example.org/a")),
            ("10.0.0.2:8080", site("http://10.0.0.2:8080/")),
            ("devbox:3000/a", site("http://devbox:3000/a")),
            ("example.com:8080", site("http://example.com:8080/")),
            ("github.com:443/acme", site("https://github.com/acme")),
            ("10.0.0.2:443", site("https://10.0.0.2/")),
            ("github.com:4430", site("http://github.com:4430/")),
            ("tic tac toe", search("tic%20tac%20toe")),
            ("note: 3000", search("note%3A%203000")),
            ("acme lab", search("acme%20lab")),
            ("vitest", search("vitest")),
            ("what is github.com", search("what%20is%20github.com")),
            ("slack://open", search("slack%3A%2F%2Fopen")),
        ] {
            assert_eq!(parse_target(raw), place, "{raw}");
        }
    }

    #[test]
    fn a_guests_url_on_the_route_reads_back_as_the_port_with_the_token_taken_off() {
        let route = Some(("https://m1-3000.preview.example/?pt_token=edge", 3000));
        assert_eq!(place_of_url("https://m1-3000.preview.example/about?tab=2&pt_token=edge#top", route), port(3000, "/about?tab=2#top"));
        assert_eq!(place_of_url("https://m1-3000.preview.example/?pt_token=t2", route), port(3000, "/"));
        assert_eq!(place_of_url("http://localhost:4000/x", route), port(4000, "/x"));
        assert_eq!(place_of_url("https://github.com/acme", route), site("https://github.com/acme"));
        assert_eq!(place_of_url("https://m1-3000.preview.example/a", None), site("https://m1-3000.preview.example/a"));
        let forward = Some(("http://localhost:41234/", 3000));
        assert_eq!(place_of_url("http://localhost:41234/about.html", forward), port(3000, "/about.html"));
    }

    #[test]
    fn a_place_links_its_loopback_url_and_shows_its_bare_form() {
        assert_eq!(place_url(&port(3000, "/a")), "http://localhost:3000/a");
        assert_eq!(place_address(&port(3000, "/a")), "localhost:3000/a");
        assert_eq!(place_url(&site("https://github.com/")), "https://github.com/");
    }

    #[test]
    fn a_host_carries_its_port_and_an_unparsable_url_is_itself() {
        assert_eq!(host_of("https://github.com/acme"), "github.com");
        assert_eq!(host_of("http://devbox:3000/a"), "devbox:3000");
        assert_eq!(host_of("not a url"), "not a url");
    }
}
