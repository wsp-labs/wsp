// SPDX-License-Identifier: AGPL-3.0-only
//! protocol/src/slate/image.ts at 65f6b89c7.
use wsp_wire::types::attachments::{IMAGE_MAX_BYTES, IMAGE_TYPE_WORDS};
use wsp_words::js::{is_js_whitespace, js_trim};
use wsp_words::words::units::{fmt_bytes, fmt_bytes_over};

use super::problems::slate_problem;
use super::SlateProblem;

pub const SLATE_IMAGE_MAX_BYTES: u64 = IMAGE_MAX_BYTES;

pub const SLATE_IMAGE_LAYOUTS: &[&str] = &["strip", "gallery", "compare"];

#[derive(Clone, Debug, PartialEq)]
pub enum SlateImageSource {
    Path(String),
    Url { url: String, domain: String },
    Problem(SlateProblem),
}

fn has_scheme(text: &str) -> bool {
    let Some(colon) = text.find(':') else {
        return false;
    };
    let scheme = &text[..colon];
    scheme.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        && scheme.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'.' | b'-'))
}

fn is_drive_path(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && matches!(bytes[2], b'\\' | b'/')
}

/// A src read as a path on the thread's computer or an http(s) address with its domain, or the problem that refuses
/// it. A relative path is read under the thread's folder; `~` is not expanded, since the host and the thread's
/// computer may not share a home.
pub fn slate_image_source(src: &str) -> SlateImageSource {
    let said = js_trim(src);
    if said.is_empty() {
        return SlateImageSource::Problem(slate_problem(
            "R900",
            "an image names no file; give a path such as /tmp/home.png or an https address",
            None,
        ));
    }
    if has_scheme(said) && !is_drive_path(said) {
        let Ok(url) = url::Url::parse(said) else {
            return SlateImageSource::Problem(slate_address_refused(said, "is not an address wsp can read"));
        };
        if !matches!(url.scheme(), "http" | "https") {
            return SlateImageSource::Problem(slate_address_refused(
                said,
                "is not an http or https address; an image is a path on the thread's computer or a web address",
            ));
        }
        if !url.username().is_empty() || url.password().is_some_and(|password| !password.is_empty()) {
            return SlateImageSource::Problem(slate_address_refused(said, "carries a login, which wsp never sends"));
        }
        return SlateImageSource::Url { url: url.as_str().to_string(), domain: url.host_str().unwrap_or_default().to_lowercase() };
    }
    if said.starts_with('~') {
        return SlateImageSource::Problem(slate_problem(
            "R900",
            &format!("{said} starts at a home folder, which wsp does not guess; write the whole path"),
            None,
        ));
    }
    SlateImageSource::Path(said.to_string())
}

/// The refusal of a file over the cap, said with its size, or with the cap alone where the bytes stopped coming at it.
pub fn slate_image_too_big(shown: &str, bytes: Option<u64>) -> SlateProblem {
    let size = match bytes {
        None => "more than".to_string(),
        Some(bytes) => format!("{}, over", fmt_bytes_over(bytes as f64)),
    };
    slate_problem(
        "R915",
        &format!(
            "{shown} is {size} the {} an image may weigh; save it smaller, or show a part of it",
            fmt_bytes(SLATE_IMAGE_MAX_BYTES as f64)
        ),
        None,
    )
}

/// The refusal of bytes that are none of the image types: an SVG is named, since it is a document that can load
/// other things, and the fix is a picture of it.
pub fn slate_not_an_image(shown: &str, svg: bool) -> SlateProblem {
    let message = if svg {
        format!("{shown} is an SVG, which a slate does not show since it can load other things; save it as a PNG")
    } else {
        format!("{shown} is not a {IMAGE_TYPE_WORDS} image")
    };
    slate_problem("R916", &message, None)
}

fn has_svg_tag(text: &str) -> bool {
    text.match_indices("<svg").any(|(at, tag)| text[at + tag.len()..].chars().next().is_some_and(|c| c == '>' || is_js_whitespace(c)))
}

/// Whether the first bytes of something that is not an image read as an SVG document.
pub fn slate_looks_svg(shown: &str, head: &[u8]) -> bool {
    let name = shown.split(['?', '#']).next().unwrap_or_default().to_lowercase();
    if name.ends_with(".svg") || name.ends_with(".svgz") {
        return true;
    }
    has_svg_tag(&String::from_utf8_lossy(&head[..head.len().min(1024)]).to_lowercase())
}

/// A path that names nothing.
pub fn slate_image_missing(path: &str) -> SlateProblem {
    slate_problem("R900", &format!("no file at {path}"), None)
}

/// A path that names a folder, a device, a pipe or a socket.
pub fn slate_image_not_a_file(path: &str) -> SlateProblem {
    slate_problem("R914", &format!("{path} is not a file; an image is a regular file"), None)
}

/// An address the host will not fetch, with why.
pub fn slate_address_refused(url: &str, why: &str) -> SlateProblem {
    slate_problem("R917", &format!("{url} {why}"), None)
}

/// A list longer than its piece draws, which draws none of it; bind names the list where it is bound.
pub fn slate_list_too_long(count: usize, most: usize, noun: &str, bind: Option<&str>) -> SlateProblem {
    let like = bind.map_or(String::new(), |bind| format!(", like items={{{bind} | take({most})}}"));
    slate_problem(
        "R905",
        &format!("this list holds {count} {noun} and a slate shows at most {most}, so it shows none; show a page of them{like}"),
        None,
    )
}
