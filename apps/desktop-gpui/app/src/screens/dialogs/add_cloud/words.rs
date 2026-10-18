// SPDX-License-Identifier: AGPL-3.0-only
//! The words of Add a cloud provider: each cloud's name and key name (`PROVIDER_KEY_WORDS`), what its key step says
//! (`cloudKey.ts`), the pick steps' heads, and the few protocol words and figures the flow reads.

/// What a person calls a provider and its key, by the id the host keys it by.
pub struct Cloud {
    pub id: &'static str,
    pub name: &'static str,
    pub key_name: &'static str,
}

/// The protocol's `PROVIDER_KEY_WORDS`, which the host's provider registry reads its key names from too.
const PLACE_WORDS: &str = include_str!("../../../../../../../packages/protocol/src/place-word.ts");

/// Every row of `PROVIDER_KEY_WORDS` in its order, read off the protocol's source: `<id>: { name: "..", keyName: ".."`.
pub fn clouds() -> Vec<Cloud> {
    let quoted = |s: &'static str, key: &str| -> Option<&'static str> {
        let at = s.find(&format!("{key}: \""))? + key.len() + 3;
        Some(&s[at..at + s[at..].find('"')?])
    };
    PLACE_WORDS
        .match_indices(": { name: \"")
        .filter_map(|(at, _)| {
            let head = &PLACE_WORDS[..at];
            let id = &head[head.rfind(|c: char| !c.is_alphanumeric() && c != '_')? + 1..];
            let row = &PLACE_WORDS[at..at + PLACE_WORDS[at..].find('}')?];
            Some(Cloud { id, name: quoted(row, "name")?, key_name: quoted(row, "keyName")? })
        })
        .collect()
}

/// The cloud offered where none is named.
pub const FIRST_CLOUD: &str = "box";

/// A provider's name by its id; an id with no row reads as itself.
pub fn cloud_name(id: &str) -> String {
    clouds().into_iter().find(|c| c.id == id).map_or_else(|| id.to_owned(), |c| c.name.to_owned())
}

pub fn key_name(id: &str) -> &'static str {
    clouds().into_iter().find(|c| c.id == id).map_or("API key", |c| c.key_name)
}

/// One provider's key step: where the key is made, the preset holding every action wsp needs and those actions, and
/// what the machines cost.
pub struct KeyStep {
    pub keys_page: &'static str,
    pub preset: &'static str,
    pub actions: &'static [&'static str],
    pub cost: &'static str,
    pub from: &'static str,
}

/// `CLOUD_KEY_STEPS` (apps/web/src/settings/add/cloudKey.ts), which no host answer or shared package carries: Boat's,
/// read off its key catalog and pricing page on 2026-10-13.
pub fn key_step(id: &str) -> Option<&'static KeyStep> {
    const STEPS: &[(&str, KeyStep)] = &[(
        "box",
        KeyStep {
            keys_page: "boat.dev/dashboard?tab=api-keys",
            preset: "ci",
            actions: &[
                "sandbox.create",
                "sandbox.read",
                "sandbox.fork",
                "sandbox.resume",
                "sandbox.stop",
                "sandbox.delete-own",
                "sandbox.update",
                "exec",
                "file.read",
                "file.write",
                "host",
                "snapshot.read",
                "snapshot.write",
                "account.read",
            ],
            cost: "Boat bills by the second, only while something runs. The $20 plan includes $20 of that time.",
            from: "from $0.018/h",
        },
    )];
    STEPS.iter().find(|(cloud, _)| *cloud == id).map(|(_, step)| step)
}

/// `KEY_REFUSED`: the kind a refusal carries where the provider answered about the key.
pub const KEY_REFUSED: &str = "keyRefused";

/// `CLOUD_SETUP_WORDS.screen.alwaysOn` and its why.
pub const ALWAYS_ON: &str = "always on";
pub const ALWAYS_ON_WHY: &str = "wsp needs it on every image, so it stays on.";
/// `CLOUD_SETUP_WORDS.build.again`.
pub const BUILD_AGAIN: &str = "Start over";
/// `CLOUD_SETUP_WORDS.build.signingIn`: the stage the build's sign-ins fold into.
pub const SIGNING_IN: &str = "Signing in on the computer";
/// `SIGN_IN_STAGE_ID`.
pub const SIGN_IN_STAGE_ID: &str = "stage/sign-ins";
/// The first launch's screen, which the build answers from the agents configured here.
pub const FIRST_LAUNCH_SCREEN: &str = "wsp";

/// Each pick step's title and its line, `cloud` the provider's name and `here` the computer the host runs on.
pub fn pick_words(pick: &str, cloud: &str, here: &str) -> (String, String) {
    match pick {
        "agents" => {
            ("Agents".into(), format!("The coding agents to put on your image on {cloud}. They use the sign-ins on {here} through wsp."))
        }
        "tools" => ("CLIs".into(), format!("The command-line tools your agents run, to put on your image on {cloud}. Most used first.")),
        _ => (format!("Also on {here}"), format!("What a package manager put on {here}, to put on your image on {cloud} too.")),
    }
}

/// `AGENTS_PAGE_WORDS`' sign-in words.
pub const SIGNED_IN_KEY: &str = "Signed in with an API key";
pub const SIGNED_IN_SUBSCRIPTION: &str = "Signed in with a subscription";
pub const SIGNED_IN_WITH: &str = "Signed in with ";

pub use crate::ui::fmt_bytes;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clouds_are_the_protocols() {
        let clouds: Vec<(&str, &str, &str)> = clouds().iter().map(|c| (c.id, c.name, c.key_name)).collect();
        assert_eq!(clouds, [("box", "Boat", "Boat API key"), ("solari", "Solari", "Solari API key")]);
    }

    #[test]
    fn bytes_read_as_the_web_writes_them() {
        assert_eq!(fmt_bytes(512), "512 B");
        assert_eq!(fmt_bytes(3 * 1024 * 1024), "3 MB");
        assert_eq!(fmt_bytes(1536 * 1024 * 1024), "1.5 GB");
        assert_eq!(fmt_bytes(2 * 1024 * 1024 * 1024), "2 GB");
    }
}
