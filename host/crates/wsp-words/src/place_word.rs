// SPDX-License-Identifier: AGPL-3.0-only
//! What a person calls a provider and its key, how a word names a place, and the refusals of a rename or a login
//! change on a computer. Port of the functions of packages/protocol/src/place-word.ts at a3b870a6e (its flat
//! constants, HERE_PLACE_ID and SOLARI_CONSOLE, are wsp_wire::types::place_word). Each TS statement stands as a
//! comment above its stub. See spec/place_word.md.

use wsp_wire::types::place_word::{HERE_PLACE_ID, SOLARI_CONSOLE};

/// What a person calls a provider, its key and where they get one (place-word.ts:12-19).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProviderKeyWords {
    pub name: &'static str,
    pub key_name: &'static str,
    pub key_console: Option<&'static str>,
    /// `subThreads: true`; false where the TS leaves it out.
    pub sub_threads: bool,
}

/// `PROVIDER_KEY_WORDS[provider]` (place-word.ts:25-35): box is Boat (key name "Boat API key", console
/// "boat.dev/dashboard", sub-threads only); solari is Solari ("Solari API key", SOLARI_CONSOLE). The TS drops solari in
/// a public build (`__WSP_CLOUD__` false); the Rust host is the cloud build and keeps it. Any other word: None.
// export const PROVIDER_KEY_WORDS: Record<string, ProviderKeyWords> = {
//   box: { name: "Boat", keyName: "Boat API key", keyConsole: "boat.dev/dashboard", subThreads: true },
//   ...(typeof __WSP_CLOUD__ === "boolean" && !__WSP_CLOUD__ ? {} : cloudOnlyKeyWords()),
// };
pub fn provider_key_words(provider: &str) -> Option<ProviderKeyWords> {
    match provider {
        "box" => {
            Some(ProviderKeyWords { name: "Boat", key_name: "Boat API key", key_console: Some("boat.dev/dashboard"), sub_threads: true })
        }
        "solari" => {
            Some(ProviderKeyWords { name: "Solari", key_name: "Solari API key", key_console: Some(SOLARI_CONSOLE), sub_threads: false })
        }
        _ => None,
    }
}

/// The provider words in the record's order: box, then solari.
pub const PROVIDER_KEY_IDS: [&str; 2] = ["box", "solari"];

// place-word.ts:38
// export const providerKeyName = (provider: string): string => PROVIDER_KEY_WORDS[provider]?.name ?? provider;
pub fn provider_key_name(provider: &str) -> String {
    provider_key_words(provider).map_or(provider, |w| w.name).to_string()
}

// place-word.ts:41
// export const subThreadsOnly = (place: string): boolean => PROVIDER_KEY_WORDS[place]?.subThreads === true;
pub fn sub_threads_only(place: &str) -> bool {
    provider_key_words(place).is_some_and(|w| w.sub_threads)
}

/// A place as the words below read it: its id, its name and its kind word where it has one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlaceRef {
    pub id: String,
    pub name: String,
    pub kind: Option<String>,
}

fn is_provider(place: &PlaceRef) -> bool {
    place.kind.as_deref() == Some("provider")
}

// place-word.ts:44
// export const placeSaid = (place: { name: string; kind?: string }): string => (place.kind === "provider" ? providerKeyName(place.name) : place.name);
pub fn place_said(place: &PlaceRef) -> String {
    if is_provider(place) {
        provider_key_name(&place.name)
    } else {
        place.name.clone()
    }
}

/// The first id a word names as a provider, case aside.
// place-word.ts:47
// export const cloudNamed = (ids: readonly string[], word: string): string | undefined => ids.find(id => namesPlace({ id, name: id, kind: "provider" }, word));
pub fn cloud_named(ids: &[impl AsRef<str>], word: &str) -> Option<String> {
    ids.iter()
        .map(AsRef::as_ref)
        .find(|id| names_place(&PlaceRef { id: id.to_string(), name: id.to_string(), kind: Some("provider".into()) }, word))
        .map(String::from)
}

/// Whether the word names this place: its id or name exactly, or, for a provider, its id, name or provider name with
/// case folded by `to_lowercase` (JavaScript's `toLowerCase`, rule 8.9).
// place-word.ts:53
// export const namesPlace = (place: { id: string; name: string; kind?: string }, word: string): boolean =>
//   word === place.id || word === place.name || (place.kind === "provider" && [place.id, place.name, providerKeyName(place.name)].some(name => name.toLowerCase() === word.toLowerCase()));
pub fn names_place(place: &PlaceRef, word: &str) -> bool {
    if word == place.id || word == place.name {
        return true;
    }
    if !is_provider(place) {
        return false;
    }
    let folded = word.to_lowercase();
    [place.id.as_str(), place.name.as_str(), &provider_key_name(&place.name)].iter().any(|name| name.to_lowercase() == folded)
}

/// A refusal's two halves, what happened and what to do, for a caller to throw as one usage refusal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefusalHalves {
    pub happened: String,
    pub fix: String,
}

fn halves(happened: impl Into<String>, fix: impl Into<String>) -> RefusalHalves {
    RefusalHalves { happened: happened.into(), fix: fix.into() }
}

// place-word.ts:62
// export function placeRenameRefusal(place: { id: string; name: string; kind?: string }, name: string, others: readonly { id: string; name: string; kind?: string }[]): RefusalHalves | undefined {
//   if (place.id === HERE_PLACE_ID || place.kind === "provider") return { happened: `${place.name} keeps its own name`, fix: "Rename one of the computers you added instead." };
//   if (name === "") return { happened: "a computer's name cannot be blank", fix: "Give it a name with a letter or a digit in it." };
//   return others.some(o => o.id !== place.id && namesPlace(o, name)) ? { happened: `${name} is already the name of another computer`, fix: "Pick a name no other computer goes by." } : undefined;
// }
pub fn place_rename_refusal(place: &PlaceRef, name: &str, others: &[PlaceRef]) -> Option<RefusalHalves> {
    if place.id == HERE_PLACE_ID || is_provider(place) {
        return Some(halves(format!("{} keeps its own name", place.name), "Rename one of the computers you added instead."));
    }
    if name.is_empty() {
        return Some(halves("a computer's name cannot be blank", "Give it a name with a letter or a digit in it."));
    }
    others
        .iter()
        .any(|o| o.id != place.id && names_place(o, name))
        .then(|| halves(format!("{name} is already the name of another computer"), "Pick a name no other computer goes by."))
}

/// Whether `ssh` is `user@host` as `^[\w.-]+@[\w.:\[\]-]+$` reads it, with `\w` ASCII only.
fn is_ssh_login(ssh: &str) -> bool {
    let Some((user, host)) = ssh.split_once('@') else { return false };
    let plain = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-';
    let part = |s: &str, extra: &str| !s.is_empty() && s.chars().all(|c| plain(c) || extra.contains(c));
    part(user, "") && part(host, ":[]")
}

/// `ssh` passes when the whole of it matches `^[\w.-]+@[\w.:\[\]-]+$` (`\w` is ASCII letters, digits and `_`).
// place-word.ts:69
// export function placeSshRefusal(place: { id: string; name: string; kind?: string }, ssh: string): RefusalHalves | undefined {
//   if (place.id === HERE_PLACE_ID || place.kind === "provider") return { happened: `${place.name} is reached over no ssh login of yours`, fix: "Give a new login to one of the computers you added instead." };
//   const fix = "Write it as the user and the host ssh logs in with, root@hetzner for one.";
//   if (ssh === "") return { happened: "an ssh login cannot be blank", fix };
//   return /^[\w.-]+@[\w.:\[\]-]+$/.test(ssh) ? undefined : { happened: `${ssh} is not an ssh login like root@host`, fix };
// }
pub fn place_ssh_refusal(place: &PlaceRef, ssh: &str) -> Option<RefusalHalves> {
    if place.id == HERE_PLACE_ID || is_provider(place) {
        return Some(halves(
            format!("{} is reached over no ssh login of yours", place.name),
            "Give a new login to one of the computers you added instead.",
        ));
    }
    let fix = "Write it as the user and the host ssh logs in with, root@hetzner for one.";
    if ssh.is_empty() {
        return Some(halves("an ssh login cannot be blank", fix));
    }
    (!is_ssh_login(ssh)).then(|| halves(format!("{ssh} is not an ssh login like root@host"), fix))
}

// place-word.ts:76
// export const placeSshOtherRefusal = (ssh: string, name: string): RefusalHalves => ({ happened: `${ssh} reaches a computer that is not ${name}, so nothing was saved`, fix: `Give the login that reaches ${name} itself.` });
pub fn place_ssh_other_refusal(ssh: &str, name: &str) -> RefusalHalves {
    halves(
        format!("{ssh} reaches a computer that is not {name}, so nothing was saved"),
        format!("Give the login that reaches {name} itself."),
    )
}

// place-word.ts:78
// export const placeSshUncheckedRefusal = (ssh: string, name: string, why: string): RefusalHalves => ({
//   happened: `${ssh} could not be checked as ${name}, so nothing was saved: ${why}`,
//   fix: `Check that ssh ${ssh} logs in as root, or through a sudo that asks no password, and try again.`,
// });
pub fn place_ssh_unchecked_refusal(ssh: &str, name: &str, why: &str) -> RefusalHalves {
    halves(
        format!("{ssh} could not be checked as {name}, so nothing was saved: {why}"),
        format!("Check that ssh {ssh} logs in as root, or through a sudo that asks no password, and try again."),
    )
}

// place-word.ts:84
// export const placeLoginElsewhere = (ssh: string, name: string, other?: string): string =>
//   other === undefined ? `${ssh} no longer reaches the computer added here as ${name}` : `${ssh} reaches the other computer added here as ${other}`;
pub fn place_login_elsewhere(ssh: &str, name: &str, other: Option<&str>) -> String {
    match other {
        None => format!("{ssh} no longer reaches the computer added here as {name}"),
        Some(other) => format!("{ssh} reaches the other computer added here as {other}"),
    }
}

// place-word.ts:88
// export const placeLoginOtherRefusal = (ssh: string, place: { id: string; name: string }, other?: string): RefusalHalves => ({
//   happened: `${placeLoginElsewhere(ssh, place.name, other)}, so nothing was changed there`,
//   fix:
//     other === undefined
//       ? `Give ${place.name} the login that reaches it with wsp computers set ${place.name} --ssh <user@host>, then try again.`
//       : `This record of ${place.name} stands beside that one; take it away with wsp remove ${place.id}.`,
// });
pub fn place_login_other_refusal(ssh: &str, place: &PlaceRef, other: Option<&str>) -> RefusalHalves {
    let happened = format!("{}, so nothing was changed there", place_login_elsewhere(ssh, &place.name, other));
    let fix = match other {
        None => format!("Give {0} the login that reaches it with wsp computers set {0} --ssh <user@host>, then try again.", place.name),
        Some(_) => format!("This record of {} stands beside that one; take it away with wsp remove {}.", place.name, place.id),
    };
    halves(happened, fix)
}

// place-word.ts:97
// export const placeLoginUncheckedRefusal = (ssh: string, name: string, why: string): RefusalHalves => ({
//   happened: `${ssh} could not be checked as ${name}, so nothing was changed there: ${why}`,
//   fix: `Check that ssh ${ssh} logs in as root, or through a sudo that takes the password you typed, and try again.`,
// });
pub fn place_login_unchecked_refusal(ssh: &str, name: &str, why: &str) -> RefusalHalves {
    halves(
        format!("{ssh} could not be checked as {name}, so nothing was changed there: {why}"),
        format!("Check that ssh {ssh} logs in as root, or through a sudo that takes the password you typed, and try again."),
    )
}

// place-word.ts:104
// export const placeLoginElsewhereRemovedLine = (ssh: string, name: string, other?: string): string =>
//   other === undefined
//     ? `${placeLoginElsewhere(ssh, name)}, so nothing was changed on the machine it reaches; ${name} is off this host, and whatever wsp left on ${name} itself stays until wsp leave runs there`
//     : `${placeLoginElsewhere(ssh, name, other)}, which stays added, so nothing was changed there; this record of ${name} is off this host`;
pub fn place_login_elsewhere_removed_line(ssh: &str, name: &str, other: Option<&str>) -> String {
    match other {
        None => format!(
            "{}, so nothing was changed on the machine it reaches; {name} is off this host, and whatever wsp left on {name} itself stays until wsp leave runs there",
            place_login_elsewhere(ssh, name, None)
        ),
        Some(_) => format!("{}, which stays added, so nothing was changed there; this record of {name} is off this host", place_login_elsewhere(ssh, name, other)),
    }
}
