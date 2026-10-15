// SPDX-License-Identifier: AGPL-3.0-only
import { posix } from "node:path";
import { shellQuote, wspBinIn } from "@wsp/protocol";
import type { DaemonPlace } from "./doctor.js";

/** The one line wsp adds to the person's own login file, and takes back out: their file is theirs, so what wsp
 * wrote is named once and found again by the file it names. Read by the deploy that writes it, by the removal that
 * runs on a machine this host reaches, and by the sweep a place runs on itself. The line reads the file before it
 * sources it: inside a workspace on a computer somebody owns that computer's /root is bound in while the wsp
 * folder under it is the workspace's own, so the file this names is not there and an unguarded line printed an
 * error on the first line of every login shell opened in one. */
export const profileSourceLine = (profileFile: string): string => `[ -f ${profileFile} ] && . ${profileFile}`;

/** A path as this place's shell scripts write it: quoted where it came from the machine, since a home with a
 * space in it would otherwise make `rm -rf /Users/Jane Doe/.wsp` two words and take /Users/Jane with it. */
export const sh = (place: DaemonPlace, path: string): string => (place.quotePaths ? shellQuote(path) : path);

/** Every line naming wsp's profile file taken out of the person's own login file, and nothing done at all where
 * they have no such file or it names none: read by the deploy before it appends the line this version spells and
 * by the removal, so a computer joined under an older spelling is left with one line of wsp's and with none once
 * wsp is off it. Their file, so the line is written back through the same path rather than moved over: a .profile
 * symlinked into a dotfiles checkout stays a symlink (measured 2026-09-11, where a move turned one into a plain
 * file). grep says by its exit code whether it selected nothing or could not read the file at all, and only the
 * first of those writes, so a read that failed leaves them neither an empty login file nor a file of wsp's beside
 * their own. Every line of it exits 0, since the deploy runs under set -e. What the take-out writes back is the
 * file line by line, so a last line the person left without a trailing newline comes back with one. */
export function unsourceStep(place: DaemonPlace, file: string): string {
  const copy = `${file}.wsp-out`;
  const named = shellQuote(place.profileFile);
  return [
    `if [ -f ${sh(place, file)} ] && grep -qF ${named} ${sh(place, file)}; then`,
    "  kept=0",
    `  grep -vF ${named} ${sh(place, file)} > ${sh(place, copy)} || kept=$?`,
    `  if [ "$kept" -le 1 ]; then cat ${sh(place, copy)} > ${sh(place, file)}; fi`,
    `  rm -f ${sh(place, copy)}`,
    "fi",
  ].join("\n");
}

/** What the login step prints for a login file it left as it was, before the path. */
export const LOGIN_KEPT_LINE = "WSP_LOGIN_KEPT";

/** Why wsp left a login file as it was, on every road that writes wsp's line or takes it out. */
export const LOGIN_LINKED_WHY = "it is a link or has a second name, and wsp writes through neither in a login file";

/** What the add's log says of that file. */
export const loginKeptLine = (file: string): string => `wsp's line was not written in ${file}: ${LOGIN_LINKED_WHY}`;

/** The test that holds where a login file is a link, dangling or not, or a file with a second name. A workspace on a
 * computer somebody owns writes in this home, and a write through either would land, as root, in a file elsewhere;
 * the leave over the link holds the same rule. */
export const linkedLoginTest = (place: DaemonPlace, file: string): string => `{ [ -L ${sh(place, file)} ] || [ -n "$(find ${sh(place, file)} -maxdepth 0 -links +1 2>/dev/null)" ]; }`;

/** wsp's line in the person's own login files as this version spells it: the lines naming wsp's profile file taken
 * out first, so a second deploy replaces the line rather than sitting beside it, then this one appended, which
 * makes the first file where the login has none and goes in each of the others only where it stands. A file that
 * is a link or has a second name is left as it is and named. Nothing at all for a place whose profile file is wsp's
 * own to load. */
export function profileSourceStep(place: DaemonPlace): string[] {
  const [first, ...rest] = place.profileSources ?? [];
  if (first === undefined) return [];
  const sourced = (file: string): string[] => [
    `if ${linkedLoginTest(place, file)}; then echo ${LOGIN_KEPT_LINE} ${sh(place, file)}; else`,
    unsourceStep(place, file),
    `printf '%s\\n' ${shellQuote(profileSourceLine(place.profileFile))} >> ${sh(place, file)}`,
    "fi",
  ];
  return [...sourced(first), ...rest.flatMap(file => [`if [ -e ${sh(place, file)} ] || [ -L ${sh(place, file)} ]; then`, ...sourced(file), "fi"])];
}

/** The login files this wsp puts on a machine: its own profile file, holding the browser shim and the display
 * every guest tool reads, and the one line of wsp's in the person's own login file that loads it. Every road that
 * puts this wsp on a computer runs these lines, the deploy at the join and the update after it, so a computer
 * joined under an older spelling takes this one without being joined again. Every line exits 0, so the same text
 * holds under the deploy's `set -e`, under the update's script, which sets none, and under the daemon's own
 * `bash -c`. */
export function loginFilesStep(place: DaemonPlace): string[] {
  return [
    // BROWSER is set by the daemon for its ptys, by this file for login shells, and in a fork's envs only when its
    // golden was sealed with the shim (claudeEnvs), never on a machine that may lack the file.
    `mkdir -p ${sh(place, posix.dirname(place.profileFile))} && printf 'export BROWSER=%s\\nunset DISPLAY\\n' ${sh(place, place.openShim)} > ${sh(place, place.profileFile)}`,
    ...(place.wsp === "bundle" ? loginWspStep(place) : []),
    // A place whose profile file is under the person's own folder is read only if their login file says so, and
    // their login file is theirs: one line of wsp's is in it however many deploys have run.
    ...profileSourceStep(place),
  ];
}

/** Where the wsp a person types at a computer somebody owns sits: inside the daemon's folder, so whatever takes the
 * bundle off takes it too. The wsp in place-bin is a thread's alone and refuses a line no turn started. */
export const loginBinIn = (dir: string): string => `${dir}/bin`;

/** The bundle's wsp under that name, appended to a login shell's PATH so a wsp of the person's own comes first: the
 * one command there that runs wsp leave, which every line naming it sends a person to type. */
function loginWspStep(place: DaemonPlace): string[] {
  const bin = loginBinIn(place.dir);
  const onPath = `case ":$PATH:" in *:${shellQuote(bin)}:*) ;; *) PATH="$PATH":${shellQuote(bin)} ;; esac`;
  return [
    `mkdir -p ${sh(place, bin)} && printf '%s\\n' '#!/bin/sh' ${shellQuote(`exec node ${shellQuote(wspBinIn(place.dir))} "$@"`)} > ${sh(place, `${bin}/wsp`)} && chmod 0755 ${sh(place, `${bin}/wsp`)}`,
    `printf '%s\\n' ${shellQuote(onPath)} >> ${sh(place, place.profileFile)}`,
  ];
}
