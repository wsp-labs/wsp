// SPDX-License-Identifier: AGPL-3.0-only
//! `places/this-computer.ts`: what this computer is called, and the key the host on it proves itself with at a join.
use std::fs;
use std::path::Path;

use wsp_core::host_id::hostname;
use wsp_store::keys::{new_place_key_pair, PlaceKeyPair};
use wsp_store::own_file::write_own;

use crate::paths::{basename, dirname, join};

/// `placeNameHere` (`places/this-computer.ts:15`).
pub fn place_name_here() -> String {
    hostname().to_lowercase()
}

/// `hostNameHere` (`places/this-computer.ts:19`).
pub fn host_name_here() -> String {
    let name = hostname();
    match name.len().checked_sub(6) {
        Some(at) if name.is_char_boundary(at) && name[at..].eq_ignore_ascii_case(".local") => name[..at].to_string(),
        _ => name,
    }
}

/// `hostPlaceKeyPath` (`places/this-computer.ts:23`).
pub fn host_place_key_path(state_path: &str) -> String {
    join(&[&dirname(state_path), "place-host-key.json"])
}

/// `hostPlaceKey` (`places/this-computer.ts:32`).
pub fn host_place_key(state_path: &str) -> PlaceKeyPair {
    let path = host_place_key_path(state_path);
    if let Some(held) = fs::read_to_string(&path).ok().and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok()) {
        if let (Some(public_key), Some(pem)) = (held["publicKey"].as_str(), held["privateKeyPem"].as_str()) {
            return PlaceKeyPair { public_key: public_key.to_string(), private_key_pem: pem.to_string() };
        }
    }
    let made = new_place_key_pair();
    let text = serde_json::to_string_pretty(&serde_json::json!({"publicKey": made.public_key, "privateKeyPem": made.private_key_pem}))
        .expect("a key pair writes as JSON");
    let _ = write_own(Path::new(&dirname(&path)), &basename(&path), format!("{text}\n"));
    made
}

/// `hostKeyHere` (`places/this-computer.ts:51`).
pub fn host_key_here(_state_path: &str) -> String {
    todo!()
}

/// `placeLabelHere` (`places/this-computer.ts:58`).
pub fn place_label_here() -> Option<String> {
    todo!()
}

/// `productNameOf` (`places/this-computer.ts:72`).
pub fn product_name_of(_ioreg: &str) -> Option<String> {
    todo!()
}

/// `computerNameHere` (`places/this-computer.ts:98`).
pub fn computer_name_here() -> String {
    todo!()
}
