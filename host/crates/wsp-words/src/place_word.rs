// SPDX-License-Identifier: AGPL-3.0-only
//! What a person calls a provider and its key, how a word names a place, and the refusals of a rename or a login
//! change on a computer. Port of the functions of packages/protocol/src/place-word.ts at a3b870a6e (its flat
//! constants, HERE_PLACE_ID and SOLARI_CONSOLE, are wsp_wire::types::place_word). Each TS statement stands as a
//! comment above its stub. See spec/place_word.md.

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
    todo!()
}

/// The provider words in the record's order: box, then solari.
pub const PROVIDER_KEY_IDS: [&str; 2] = ["box", "solari"];

// place-word.ts:38
// export const providerKeyName = (provider: string): string => PROVIDER_KEY_WORDS[provider]?.name ?? provider;
pub fn provider_key_name(provider: &str) -> String {
    todo!()
}

// place-word.ts:41
// export const subThreadsOnly = (place: string): boolean => PROVIDER_KEY_WORDS[place]?.subThreads === true;
pub fn sub_threads_only(place: &str) -> bool {
    todo!()
}

/// A place as the words below read it: its id, its name and its kind word where it has one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlaceRef {
    pub id: String,
    pub name: String,
    pub kind: Option<String>,
}

// place-word.ts:44
// export const placeSaid = (place: { name: string; kind?: string }): string => (place.kind === "provider" ? providerKeyName(place.name) : place.name);
pub fn place_said(place: &PlaceRef) -> String {
    todo!()
}

/// The first id a word names as a provider, case aside.
// place-word.ts:47
// export const cloudNamed = (ids: readonly string[], word: string): string | undefined => ids.find(id => namesPlace({ id, name: id, kind: "provider" }, word));
pub fn cloud_named(ids: &[impl AsRef<str>], word: &str) -> Option<String> {
    todo!()
}

/// Whether the word names this place: its id or name exactly, or, for a provider, its id, name or provider name with
/// case folded by `to_lowercase` (JavaScript's `toLowerCase`, rule 8.9).
// place-word.ts:53
// export const namesPlace = (place: { id: string; name: string; kind?: string }, word: string): boolean =>
//   word === place.id || word === place.name || (place.kind === "provider" && [place.id, place.name, providerKeyName(place.name)].some(name => name.toLowerCase() === word.toLowerCase()));
pub fn names_place(place: &PlaceRef, word: &str) -> bool {
    todo!()
}

/// A refusal's two halves, what happened and what to do, for a caller to throw as one usage refusal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefusalHalves {
    pub happened: String,
    pub fix: String,
}

// place-word.ts:62
// export function placeRenameRefusal(place: { id: string; name: string; kind?: string }, name: string, others: readonly { id: string; name: string; kind?: string }[]): RefusalHalves | undefined {
//   if (place.id === HERE_PLACE_ID || place.kind === "provider") return { happened: `${place.name} keeps its own name`, fix: "Rename one of the computers you added instead." };
//   if (name === "") return { happened: "a computer's name cannot be blank", fix: "Give it a name with a letter or a digit in it." };
//   return others.some(o => o.id !== place.id && namesPlace(o, name)) ? { happened: `${name} is already the name of another computer`, fix: "Pick a name no other computer goes by." } : undefined;
// }
pub fn place_rename_refusal(place: &PlaceRef, name: &str, others: &[PlaceRef]) -> Option<RefusalHalves> {
    todo!()
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
    todo!()
}

// place-word.ts:76
// export const placeSshOtherRefusal = (ssh: string, name: string): RefusalHalves => ({ happened: `${ssh} reaches a computer that is not ${name}, so nothing was saved`, fix: `Give the login that reaches ${name} itself.` });
pub fn place_ssh_other_refusal(ssh: &str, name: &str) -> RefusalHalves {
    todo!()
}

// place-word.ts:78
// export const placeSshUncheckedRefusal = (ssh: string, name: string, why: string): RefusalHalves => ({
//   happened: `${ssh} could not be checked as ${name}, so nothing was saved: ${why}`,
//   fix: `Check that ssh ${ssh} logs in as root, or through a sudo that asks no password, and try again.`,
// });
pub fn place_ssh_unchecked_refusal(ssh: &str, name: &str, why: &str) -> RefusalHalves {
    todo!()
}

// place-word.ts:84
// export const placeLoginElsewhere = (ssh: string, name: string, other?: string): string =>
//   other === undefined ? `${ssh} no longer reaches the computer added here as ${name}` : `${ssh} reaches the other computer added here as ${other}`;
pub fn place_login_elsewhere(ssh: &str, name: &str, other: Option<&str>) -> String {
    todo!()
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
    todo!()
}

// place-word.ts:97
// export const placeLoginUncheckedRefusal = (ssh: string, name: string, why: string): RefusalHalves => ({
//   happened: `${ssh} could not be checked as ${name}, so nothing was changed there: ${why}`,
//   fix: `Check that ssh ${ssh} logs in as root, or through a sudo that takes the password you typed, and try again.`,
// });
pub fn place_login_unchecked_refusal(ssh: &str, name: &str, why: &str) -> RefusalHalves {
    todo!()
}

// place-word.ts:104
// export const placeLoginElsewhereRemovedLine = (ssh: string, name: string, other?: string): string =>
//   other === undefined
//     ? `${placeLoginElsewhere(ssh, name)}, so nothing was changed on the machine it reaches; ${name} is off this host, and whatever wsp left on ${name} itself stays until wsp leave runs there`
//     : `${placeLoginElsewhere(ssh, name, other)}, which stays added, so nothing was changed there; this record of ${name} is off this host`;
pub fn place_login_elsewhere_removed_line(ssh: &str, name: &str, other: Option<&str>) -> String {
    todo!()
}
