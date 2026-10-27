// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `cloud-configs.ts`: what a copy to a provider's machine cuts out of the configs it carries.

/// The `Cut` of `cloudCut`, not exported in the TS (`cloud-configs.ts:57`): the text that lands and the names of what stayed.
#[derive(Clone, Debug, PartialEq)]
pub struct Cut {
    pub text: String,
    pub names: Vec<String>,
}

/// `stayedLine` (`cloud-configs.ts:30`).
pub fn stayed_line(_names: &[String]) -> String {
    todo!()
}

/// `SIGN_INS_STAY` (`cloud-configs.ts:31`).
pub const SIGN_INS_STAY: &str = "mcp-remote's sign-ins stay on this computer; sign in on the machine";

/// `FISH_VARIABLES_STAY` (`cloud-configs.ts:32`).
pub const FISH_VARIABLES_STAY: &str = "fish keeps its universal variables, secrets among them, here; they stay on this computer";

/// `PRIVATE_KEY_STAYS` (`cloud-configs.ts:33`).
pub const PRIVATE_KEY_STAYS: &str = "holds a private key, which stays on this computer";

/// `carriesSecret` (`cloud-configs.ts:94`).
pub fn carries_secret(_address: &str) -> bool {
    todo!()
}

/// `leftWhole` (`cloud-configs.ts:341`).
pub fn left_whole(_name: &str, _text: Option<&str>) -> Option<String> {
    todo!()
}

/// `cloudCut` (`cloud-configs.ts:349`).
pub fn cloud_cut(_path: &str, _text: &str, _home: &str) -> Cut {
    todo!()
}
