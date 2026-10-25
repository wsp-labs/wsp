// SPDX-License-Identifier: AGPL-3.0-only
//! What the system package manager has that a person chose themselves: marked manual, of a priority the distro does
//! not need, shipping a command. Read-only: apt-mark, dpkg-query and dpkg's own records. Port of
//! packages/collect/src/detect/apt.ts at a3b870a6e; the unit is apt_tools and its spec is spec/apt_tools.md.
use indexmap::IndexMap;
use wsp_words::exit::HostError;

use crate::detect::tools::Pkg;
use crate::host::Host;

/// Where dpkg keeps what each package installed (apt.ts:15).
pub const DPKG_INFO: &str = "/var/lib/dpkg/info";

/// The priorities a person's own choice has (apt.ts:19).
pub const CHOSEN: [&str; 2] = ["optional", "extra"];

/// Packages marked manual that nobody chose (apt.ts:23).
pub const NOT_A_CHOICE: [&str; 4] = ["sudo", "unminimize", "lsb-release", "apt-transport-https"];

/// A file directly under /usr/bin or /bin, as a JavaScript regex source (apt.ts:27).
pub const COMMAND_PATH: &str = r"^\/(usr\/)?bin\/[^/]+$";

// apt.ts:31
// /** `<package> <field>` lines as dpkg-query writes them, by package; a line naming no value says nothing and is
//  * dropped. A package installed for two architectures is one entry, since `${Package}` carries no architecture. */
// export function parseDpkgFields(text: string): Map<string, string> {
//   const out = new Map<string, string>();
//   for (const line of text.split("\n")) {
//     const m = /^(\S+)\s+(\S+)\s*$/.exec(line);
//     if (m?.[1] === undefined || m[2] === undefined) continue;
//     out.set(m[1], m[2]);
//   }
//   return out;
// }
/// `<package> <field>` lines by package, in first-seen order; a later line for the same package replaces the value
/// and keeps the position, as a JS Map's set does.
pub fn parse_dpkg_fields(text: &str) -> IndexMap<String, String> {
    todo!()
}

// apt.ts:42
// /** The names `apt-mark showmanual` answers with, in its own order. */
// export function parseAptManual(text: string): string[] {
//   return text.split("\n").map(l => l.trim()).filter(l => l !== "");
// }
pub fn parse_apt_manual(text: &str) -> Vec<String> {
    todo!()
}

// Private helpers the port keeps (names in snake case):
// apt.ts:49
// /** Every installed package's file list by package name: dpkg names the file `<pkg>.list`, or `<pkg>:<arch>.list`
//  * for a package installed for a named architecture. One directory read, since a `dpkg -L` per candidate is a
//  * process per candidate. */
// async function fileLists(host: Host): Promise<Map<string, string>> {
//   const out = new Map<string, string>();
//   for (const name of await host.fs.list(DPKG_INFO)) {
//     if (!name.endsWith(".list")) continue;
//     const pkg = name.slice(0, -".list".length).split(":")[0];
//     if (pkg !== undefined && pkg !== "") out.set(pkg, `${DPKG_INFO}/${name}`);
//   }
//   return out;
// }
// apt.ts:61
// /** Whether the package put a command on PATH, read from dpkg's list for it one line at a time; a package dpkg has
//  * no list for says nothing about itself and is left out. */
// async function shipsCommand(host: Host, file: string | undefined): Promise<boolean> {
//   if (file === undefined) return false;
//   for await (const line of host.fs.lines(file)) if (COMMAND_PATH.test(line.trim())) return true;
//   return false;
// }

// apt.ts:70
// /** The apt packages this computer's owner chose: marked manual, of a priority the distro does not need, and
//  * shipping a command. What the catalog or another rung already carries is dropped by whoever asked, so both
//  * callers read one list. The caller has already found apt-mark on PATH. */
// export async function aptPackages(host: Host): Promise<Pkg[]> {
//   const manual = parseAptManual((await host.exec.run("apt-mark", ["showmanual"])) ?? "");
//   const mine = manual.filter(name => !NOT_A_CHOICE.has(name));
//   if (mine.length === 0) return [];
//   const priority = parseDpkgFields((await host.exec.run("dpkg-query", ["-Wf", "${Package} ${Priority}\n"])) ?? "");
//   const lists = await fileLists(host);
//   const out: Pkg[] = [];
//   for (const name of mine.filter(n => CHOSEN.has(priority.get(n) ?? ""))) {
//     if (await shipsCommand(host, lists.get(name))) out.push({ name });
//   }
//   return out;
// }
/// The caller has already found apt-mark on PATH. A list file whose `lines` stream yields an `Err` part way fails the
/// whole read with `HostError::new(err.to_string())`, the TS iterator's throw (coordinator's ruling on #2446).
pub async fn apt_packages(host: &Host) -> Result<Vec<Pkg>, HostError> {
    todo!()
}

// apt.ts:85
// /** What each of these packages takes on this computer, from dpkg's own record: its Installed-Size is kibibytes, as
//  * `du -sk` is for the other managers. One run for all of them, and none when nothing needs a size. */
// export async function aptSizes(host: Host, names: readonly string[]): Promise<Map<string, number>> {
//   if (names.length === 0) return new Map();
//   const fields = parseDpkgFields((await host.exec.run("dpkg-query", ["-Wf", "${Package} ${Installed-Size}\n"])) ?? "");
//   const out = new Map<string, number>();
//   for (const name of names) {
//     const kib = Number(fields.get(name));
//     if (Number.isFinite(kib) && kib > 0) out.set(name, kib * 1024);
//   }
//   return out;
// }
/// Bytes by package name, in the order of `names`; JavaScript's `Number()` reads the field, so the value is an f64.
pub async fn apt_sizes(host: &Host, names: &[String]) -> IndexMap<String, f64> {
    todo!()
}
