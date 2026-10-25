// SPDX-License-Identifier: AGPL-3.0-only
//! Accepted-difference class `idna-version`: the `url` crate maps and validates a host's international characters by its own
//! Unicode table, which is not node 22.23.4's (about 23,500 code points answer differently, mostly the newer ones, and about 150
//! that node takes the crate refuses over a Bidi rule node does not apply). Each case below is one the review found, with
//! node's answer in its comment and the crate's pinned: when a `url` update moves the table, the case that flips names it.
//! The differences are about which odd host a clone address or a join address takes; none lets a different host through, since
//! a code the crate maps away leaves the same host the TS dialled.

use wsp_words::app_ports::{join_address_word, served_hostname, ws_url_of};
use wsp_words::projects::clone_url_refusal;

fn refused(word: &str) -> String {
    format!("{word} is not a repo address wsp clones; give its https or ssh url, or owner/repo")
}

// node: cloneUrlRefusal("https://github.comㅤ/x/y") is the refusal sentence (U+3164, HANGUL FILLER, is not valid in its table).
#[test]
fn a_host_holding_a_hangul_filler_is_taken_here_and_refused_by_node() {
    assert_eq!(clone_url_refusal("https://github.com\u{3164}/x/y"), None);
}

// node: cloneUrlRefusal("https://github.com٠/x/y") is undefined (it applies no Bidi rule); the crate refuses the label.
#[test]
fn a_label_ending_in_an_arabic_indic_digit_is_refused_here_and_taken_by_node() {
    let word = "https://github.com\u{660}/x/y";
    assert_eq!(clone_url_refusal(word), Some(refused(word)));
}

// node: wsUrlOf("http://ẞ.de") is "ws://ss.de/ws"; the crate keeps the letter and writes its punycode.
#[test]
fn a_capital_sharp_s_is_punycode_here_and_ss_for_node() {
    assert_eq!(ws_url_of("http://\u{1E9E}.de").unwrap(), "ws://xn--zca.de/ws");
}

// node: wsUrlOf("https://aӀb.com/x/y") throws "Invalid URL" (U+04C0 has no mapping in its table); the crate takes it.
#[test]
fn a_letter_node_has_no_mapping_for_is_taken_here() {
    assert!(ws_url_of("https://a\u{4C0}b.com/x/y").is_ok());
}

// node: servedHostname("http://xn--ab-.com") is "xn--ab-.com" and joinAddressWord the same; the crate answers the same.
#[test]
fn a_punycode_label_node_takes_is_taken_here_too() {
    assert_eq!(served_hostname("http://xn--ab-.com").as_deref(), Some("xn--ab-.com"));
    assert_eq!(join_address_word("http://xn--ab-.com"), "xn--ab-.com");
}
