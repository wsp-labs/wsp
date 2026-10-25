// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `joins`: array min 1
/// - `expiresAt`: string datetime
impl Check for JoinMint {
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::items_at_least(self.joins.len(), 1).map_err(|e| e.under("joins"))?;
        self.joins.check().map_err(|e| e.under("joins"))?;
        datetime_string(&self.expires_at).map_err(|e| e.under("expiresAt"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<Vec<JoinRoad>>(obj, "joins", &mut out, Self::rule_joins);
        crate::types::keys::req_key_with::<String>(obj, "expiresAt", &mut out, Self::rule_expires_at);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "joins" => <Vec<JoinRoad> as Check>::describe(rest),
                "expiresAt" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl JoinMint {
    fn rule_joins(x: &Vec<JoinRoad>) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::items_at_least((*x).len(), 1)?;
        Ok(())
    }
    fn rule_expires_at(x: &String) -> Result<(), crate::types::check::SchemaError> {
        datetime_string(x)?;
        Ok(())
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `alias`: string min 1
/// - `alias`: string max 300
/// - `hostName`: string max 300
/// - `user`: string max 300
/// - `port`: int min 1
/// - `port`: int max 65535
impl Check for SshHostSuggestion {
    fn check(&self) -> Result<(), SchemaError> {
        text_at_least(&self.alias, 1).map_err(|e| e.under("alias"))?;
        text_at_most(&self.alias, 300).map_err(|e| e.under("alias"))?;
        if let Some(host_name) = &self.host_name {
            text_at_most(host_name, 300).map_err(|e| e.under("hostName"))?;
        }
        if let Some(user) = &self.user {
            text_at_most(user, 300).map_err(|e| e.under("user"))?;
        }
        if let Some(port) = self.port {
            num_at_least(port.as_f64(), 1.0).map_err(|e| e.under("port"))?;
            num_at_most(port.as_f64(), 65535.0).map_err(|e| e.under("port"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<String>(obj, "alias", &mut out, Self::rule_alias);
        crate::types::keys::opt_key_with::<String>(obj, "hostName", &mut out, Self::rule_host_name);
        crate::types::keys::opt_key_with::<String>(obj, "user", &mut out, Self::rule_user);
        crate::types::keys::opt_key_with::<JsInt>(obj, "port", &mut out, Self::rule_port);
        crate::types::keys::req_key::<SshHostSuggestionFrom>(obj, "from", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "alias" => <String as Check>::describe(rest),
                "hostName" => <Option<String> as Check>::describe(rest),
                "user" => <Option<String> as Check>::describe(rest),
                "port" => <Option<i64> as Check>::describe(rest),
                "from" => <SshHostSuggestionFrom as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SshHostSuggestion {
    fn rule_alias(x: &String) -> Result<(), crate::types::check::SchemaError> {
        text_at_least(x, 1)?;
        text_at_most(x, 300)?;
        Ok(())
    }
    fn rule_host_name(x: &Option<String>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(host_name) = x {
            text_at_most(host_name, 300)?;
        }
        Ok(())
    }
    fn rule_user(x: &Option<String>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(user) = x {
            text_at_most(user, 300)?;
        }
        Ok(())
    }
    fn rule_port(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(port) = *x {
            num_at_least(port.as_f64(), 1.0)?;
            num_at_most(port.as_f64(), 65535.0)?;
        }
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for TwoPartRefusal {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "what", &mut out);
        crate::types::keys::req_key::<String>(obj, "fix", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "what" => <String as Check>::describe(rest),
                "fix" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

// place-link.ts:30
// export const engineWord = (hasDocker: boolean, hasPodman: boolean): PlaceEngine => (hasDocker ? "docker" : hasPodman ? "podman" : "none");
pub fn engine_word(has_docker: bool, has_podman: bool) -> PlaceEngine {
    if has_docker {
        PlaceEngine::Docker
    } else if has_podman {
        PlaceEngine::Podman
    } else {
        PlaceEngine::None
    }
}

// place-link.ts:213
// /** The refusal a line gets for aiming at a host it holds no key for: a record somebody edited by hand, or a turn
//  * launched by a host older than this one. `where` names which. */
// export const hostNoKeyLine = (where: string): string =>
//   `${where} names a host and no key for it, so this computer cannot tell which host it would be sending its token to; run wsp hosts to read the account's hosts again`;
pub fn host_no_key_line(r#where: &str) -> String {
    format!("{where} names a host and no key for it, so this computer cannot tell which host it would be sending its token to; run wsp hosts to read the account's hosts again")
}

// place-link.ts:227
// /** The refusal for a word two computers on this host answer to: ids tell them apart, and the person picks one.
//  * `typed` is the word that was written, since more than one word names a computer and each says its own back. A
//  * relink cannot take another computer's name, so two by one name are two a person joined under one word, and
//  * every road that resolves a word reads this one sentence. */
// export const twoPlacesRefusal = (typed: string, ids: readonly string[]): string =>
//   `${typed}: this host holds ${ids.length} places by that name; name one by its id (${ids.join(", ")}).`;
pub fn two_places_refusal(typed: &str, ids: &[String]) -> String {
    format!("{typed}: this host holds {} places by that name; name one by its id ({}).", ids.len(), ids.join(", "))
}

// place-link.ts:247
// /** The refusal a joining computer prints when the host at that address could not prove the key this computer
//  * learned at join, so nothing of this computer's went to it. */
// export const hostKeyRefusal = (url: string): string => `the host at ${url} did not prove the key this computer learned at join; nothing was sent to it`;
pub fn host_key_refusal(url: &str) -> String {
    format!("the host at {url} did not prove the key this computer learned at join; nothing was sent to it")
}

// place-link.ts:260
// /** What a remove took off that computer by its agent's own command: a plugin the setup put on there. */
// export const pluginOffLine = (name: string): string => `plugin ${name}`;
pub fn plugin_off_line(name: &str) -> String {
    format!("plugin {name}")
}

// place-link.ts:263
// /** What a remove says of the plugins the setup put on that computer and could not take off: they stay there. */
// export const pluginsKeptLine = (name: string, plugins: readonly string[]): string =>
//   `${plugins.join(", ")} ${plugins.length === 1 ? "is" : "are"} still on ${name}: wsp could not take ${plugins.length === 1 ? "it" : "them"} off`;
pub fn plugins_kept_line(name: &str, plugins: &[String]) -> String {
    let (verb, them) = if plugins.len() == 1 { ("is", "it") } else { ("are", "them") };
    format!("{} {verb} still on {name}: wsp could not take {them} off", plugins.join(", "))
}

// place-link.ts:267
// /** What a place that is connected but has never said which port its daemon bound is refused with: a pane needs
//  * that port to carry to, and only that computer knows it. */
// export const placeNoDaemonPortLine = (name: string): string => `${name} is connected but has not said which port its daemon is on, so nothing can carry a pane to it yet; it says so on its next link`;
pub fn place_no_daemon_port_line(name: &str) -> String {
    format!("{name} is connected but has not said which port its daemon is on, so nothing can carry a pane to it yet; it says so on its next link")
}

// place-link.ts:271
// /** What an install is refused with when the computer took the daemon and never dialled back: the join landed, so
//  * the computer belongs to this wsp, and what is missing is a road from it to here. */
// export const placeNoLinkLine = (name: string): string => `${name} took the daemon and has not dialled this host yet; check that it can reach this computer on the address it was given, and wsp computers shows it the moment it does`;
pub fn place_no_link_line(name: &str) -> String {
    format!("{name} took the daemon and has not dialled this host yet; check that it can reach this computer on the address it was given, and wsp computers shows it the moment it does")
}

// place-link.ts:275
// /** What a stage reads while the computer it is running on has no link: the requests behind it are held until that
//  * computer opens a socket again, and a stage with no line of its own reads as one that stopped. */
// export const placeDialBackLine = (name: string): string => `waiting for ${name} to dial back`;
pub fn place_dial_back_line(name: &str) -> String {
    format!("waiting for {name} to dial back")
}

// place-link.ts:279
// /** A Browser pane asking for a port of a computer the person joined that this computer would need root to open. */
// export const paneForwardFloorLine = (port: number): string => `localhost:${port} is below 1024, which this computer opens only as root; serve it on a port from 1024 up`;
pub fn pane_forward_floor_line(port: i64) -> String {
    format!("localhost:{port} is below 1024, which this computer opens only as root; serve it on a port from 1024 up")
}

// place-link.ts:282
// /** A Browser pane asking for one port more than a workspace may hold open on this computer. */
// export const paneForwardCapLine = (name: string, cap: number): string => `${name} already has ${cap} ports open on this computer; stop one in Ports first`;
pub fn pane_forward_cap_line(name: &str, cap: i64) -> String {
    format!("{name} already has {cap} ports open on this computer; stop one in Ports first")
}

// place-link.ts:285
// /** A Browser pane asking again for a port whose forward a quiet hour ended. */
// export const paneForwardQuietLine = (port: number): string => `localhost:${port} closed after an hour with nothing connecting to it; open the address again to forward it`;
pub fn pane_forward_quiet_line(port: i64) -> String {
    format!("localhost:{port} closed after an hour with nothing connecting to it; open the address again to forward it")
}

// place-link.ts:288
// /** A Browser pane asking again for a port whose forward the person stopped in Ports. */
// export const paneForwardStoppedLine = (port: number): string => `localhost:${port} was stopped in Ports; open the address again to forward it`;
pub fn pane_forward_stopped_line(port: i64) -> String {
    format!("localhost:{port} was stopped in Ports; open the address again to forward it")
}

// place-link.ts:291
// /** A pane's fetch of a port whose forward does not stand: the fetch never opens one, the pane's own ask does. */
// export const paneForwardGoneLine = (port: number): string => `localhost:${port} is not forwarded to this computer now`;
pub fn pane_forward_gone_line(port: i64) -> String {
    format!("localhost:{port} is not forwarded to this computer now")
}

// place-link.ts:294
// /** Why a build on a joined computer stopped when that computer's link went and it never dialled back in time. */
// export const placeWentAwayLine = (name: string): string => `${name} went away before the build finished`;
pub fn place_went_away_line(name: &str) -> String {
    format!("{name} went away before the build finished")
}

// place-link.ts:353
// /** A url's host as a name: lowercase, without its port or a trailing root dot. */
// export const bareHost = (host: string): string => host.replace(/:\d*$/, "").toLowerCase().replace(/\.$/, "");
pub fn bare_host(host: &str) -> String {
    let without_port = match host.rfind(':') {
        Some(at) if host[at + 1..].bytes().all(|b| b.is_ascii_digit()) => &host[..at],
        _ => host,
    };
    let lower = without_port.to_lowercase();
    lower.strip_suffix('.').unwrap_or(&lower).to_string()
}

// place-link.ts:356
// /** Whether a host is the domain or a host under it, in any case, with a port or a root dot or neither. */
// export function hostUnder(host: string, domain: string): boolean {
//   const name = bareHost(host);
//   return name === domain || name.endsWith(`.${domain}`);
// }
pub fn host_under(host: &str, domain: &str) -> bool {
    let name = bare_host(host);
    name == domain || name.ends_with(&format!(".{domain}"))
}

// place-link.ts:362
// /** Whether a host name is one of GIT_FORGE_HOSTS or under one. */
// export function isGitForge(host: string): boolean {
//   return GIT_FORGE_HOSTS.some(forge => hostUnder(host, forge));
// }
pub fn is_git_forge(host: &str) -> bool {
    GIT_FORGE_HOSTS.iter().any(|forge| host_under(host, forge))
}

// place-link.ts:396
// /** What the sheet says when somebody else already holds the door's port: a fixed port, since the place file on the
//  * other computer names it for good, so a fallback port would be a computer that can never dial back. */
// export const doorPortHeldLine = (port: number): string =>
//   `port ${port} is held by another program on this computer, so no computer you own can reach this wsp; free it and open Add a computer again`;
pub fn door_port_held_line(port: i64) -> String {
    format!("port {port} is held by another program on this computer, so no computer you own can reach this wsp; free it and open Add a computer again")
}

// place-link.ts:401
// /** Said when the door port of a host's last start is taken and it opens on another: a computer joined at the old one
//  * dials it for good. */
// export const doorPortMovedLine = (port: number): string =>
//   `port ${port}, where computers you joined last reached this wsp, is held by another program, so the door opened on another; free it and start this wsp again`;
pub fn door_port_moved_line(port: i64) -> String {
    format!("port {port}, where computers you joined last reached this wsp, is held by another program, so the door opened on another; free it and start this wsp again")
}

// place-link.ts:426
// /** The place file a text holds, or nothing when that text is not one. A file that is there and is not one reads the
//  * same as none: the one road that writes it is wsp join, and anything else there is not a place to dial with. */
// export function parsePlaceFile(text: string): PlaceFile | undefined {
//   let held: unknown;
//   try {
//     held = JSON.parse(text);
//   } catch {
//     return undefined;
//   }
//   const f = held as PlaceFile | undefined;
//   const ok =
//     typeof f === "object" &&
//     f !== null &&
//     typeof f.placeId === "string" &&
//     typeof f.name === "string" &&
//     typeof f.hostName === "string" &&
//     Array.isArray(f.hostUrls) &&
//     f.hostUrls.every(u => typeof u === "string") &&
//     typeof f.hostPublicKey === "string" &&
//     typeof f.keyPath === "string";
//   return ok ? f : undefined;
// }
pub fn parse_place_file(text: &str) -> Option<PlaceFile> {
    serde_json::from_value(crate::wire::json_text::parse_json(text)?).ok()
}

// place-link.ts:457
// /** The place file the held read found, read past what the login printed before it. */
// export const heldPlaceFile = (stdout: string): PlaceFile | undefined => parsePlaceFile(heldText(stdout) ?? "");
pub fn held_place_file(stdout: &str) -> Option<PlaceFile> {
    parse_place_file(held_text(stdout).unwrap_or_default())
}

// place-link.ts:460
// /** What the held read of a box says stands there, by the rule the join on that box refuses by: a place file that
//  * parses is a join; anything else it answered after its own line is what a join left broken, named by the path
//  * that stands. */
// export function readHeldPlace(stdout: string, home: string): { joined: PlaceFile } | { broken: string } | undefined {
//   const text = heldText(stdout);
//   if (text === undefined || text === "") return undefined;
//   const joined = parsePlaceFile(text);
//   if (joined !== undefined) return { joined };
//   const at = placeDaemonPaths(home);
//   return { broken: text.trim() === `${PLACE_LEFT_LINE} key` ? at.placeKey : at.placeFile };
// }
pub fn read_held_place(stdout: &str, home: &str) -> Option<HeldPlace> {
    let text = held_text(stdout)?;
    if text.is_empty() {
        return None;
    }
    if let Some(file) = parse_place_file(text) {
        return Some(HeldPlace::Joined(file));
    }
    let paths = wsp_frames::place_daemon_paths(std::path::Path::new(home));
    let broken = if crate::types::check::js_trim(text) == format!("{} key", crate::types::project_path::PLACE_LEFT_LINE) {
        paths.place_key
    } else {
        paths.place_file
    };
    Some(HeldPlace::Broken(broken.to_string_lossy().into_owned()))
}

// place-link.ts:472
// /** Whether a place file names one of these computers and this host's key: the one test every road over a computer's
//  * ssh login holds the machine it reached to before it changes anything there. */
// export const placeFileNames = (file: PlaceFile | undefined, placeIds: readonly string[], hostPublicKey: string): file is PlaceFile =>
//   file !== undefined && placeIds.includes(file.placeId) && file.hostPublicKey === hostPublicKey;
pub fn place_file_names(file: Option<&PlaceFile>, place_ids: &[String], host_public_key: &str) -> bool {
    file.is_some_and(|f| place_ids.contains(&f.place_id) && f.host_public_key == host_public_key)
}

// place-link.ts:477
// /** The text the file holds, which parsePlaceFile reads back. */
// export const placeFileText = (file: PlaceFile): string => `${JSON.stringify(file, null, 2)}\n`;
pub fn place_file_text(file: &PlaceFile) -> String {
    let text = serde_json::to_string_pretty(file).expect("a place file writes as JSON");
    format!("{}\n", crate::wire::json_text::restore_surrogates(&text))
}

// The shapes wsp-frames holds read every bound while deserializing, so each Check has nothing left to refuse.
impl Check for PlaceFile {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::req_key::<String>(obj, "hostName", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "hostUrls", &mut out);
        crate::types::keys::req_key::<String>(obj, "hostPublicKey", &mut out);
        crate::types::keys::req_key::<String>(obj, "keyPath", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "placeId" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "hostName" => <String as Check>::describe(rest),
                "hostUrls" => <Vec<String> as Check>::describe(rest),
                "hostPublicKey" => <String as Check>::describe(rest),
                "keyPath" => <String as Check>::describe(rest),
                "joinedAt" => <Option<serde_json::Value> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

// A word list zod reads with z.enum: serde refuses any other word, so nothing is left to check.
impl Check for PlaceEngine {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["none", "docker", "podman"]))
    }
}

impl Check for PlaceReportPlatform {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["darwin", "linux"]))
    }
}

impl Check for PlaceReportEngine {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["none", "docker", "podman"]))
    }
}

impl Check for SshHostSuggestionFrom {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["config", "known_hosts"]))
    }
}

/// helpers.ts base64(n): `z.string().max(..).regex(..).refine(..)`, each run on a string, every failure an issue.
impl<const N: usize> Check for wsp_frames::Base64Bytes<N> {
    fn pass(value: &serde_json::Value) -> Option<Vec<SchemaError>> {
        let serde_json::Value::String(text) = value else { return None };
        let digits = text.trim_end_matches('=');
        let pad = text.len() - digits.len();
        let in_alphabet = !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/');
        let len = crate::types::check::utf16_len(text);
        let pad_bytes = if text.ends_with("==") { 2 } else { usize::from(text.ends_with('=')) };
        let counts = len.is_multiple_of(4) && len / 4 * 3 - pad_bytes == N;
        Some(
            [
                text_at_most(text, N.div_ceil(3) * 4),
                regex_check(in_alphabet && pad <= 2),
                refine(counts, &format!("must be {N} bytes, base64")),
            ]
            .into_iter()
            .filter_map(Result::err)
            .collect(),
        )
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Word("string"))
    }
}
