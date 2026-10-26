// SPDX-License-Identifier: AGPL-3.0-only
//! The words of Add a cloud provider: each cloud's name and key name (`PROVIDER_KEY_WORDS`), what its key step says
//! (`cloudKey.ts`), and the protocol word the flow reads.

use crate::screens::settings::pages::computers::{ProviderWords, PROVIDERS};

/// Every cloud this build can name, in the protocol's order.
pub fn clouds() -> &'static [ProviderWords] {
    PROVIDERS
}

/// The cloud offered where none is named.
pub const FIRST_CLOUD: &str = "box";

/// A provider's name by its id; an id with no row reads as itself.
pub fn cloud_name(id: &str) -> String {
    clouds().iter().find(|c| c.id == id).map_or_else(|| id.to_owned(), |c| c.name.to_owned())
}

pub fn key_name(id: &str) -> &'static str {
    clouds().iter().find(|c| c.id == id).map_or("API key", |c| c.key_name)
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
