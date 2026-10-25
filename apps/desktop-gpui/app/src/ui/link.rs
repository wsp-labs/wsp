// SPDX-License-Identifier: AGPL-3.0-only
//! Opening a link a tool or a file handed over, which nobody here wrote: as a web page or a mail address only
//! (`restrictedHref`), so a `file:` or an app's own scheme in tool output never reaches the computer's opener.

use gpui_kit::App;

/// Whether `url` is a web page or a mail address, by its scheme in any case.
pub fn opens(url: &str) -> bool {
    let Some((scheme, _)) = url.split_once(':') else { return false };
    ["http", "https", "mailto"].iter().any(|s| scheme.eq_ignore_ascii_case(s))
}

/// Opens `url` where [`opens`] allows it; anything else is not opened.
pub fn open_link(url: &str, cx: &mut App) {
    if opens(url) {
        cx.open_url(url);
    }
}

#[cfg(test)]
mod tests {
    use super::opens;

    #[test]
    fn only_a_web_page_or_a_mail_address_opens() {
        for url in ["https://github.com/login/device", "http://localhost:3000", "HTTPS://A.B", "mailto:a@b.c"] {
            assert!(opens(url), "{url}");
        }
        for url in ["file:///etc/passwd", "javascript:alert(1)", "vscode://x", "ssh://box", "github.com/login", " https://a.b", "", "https"]
        {
            assert!(!opens(url), "{url}");
        }
    }
}
