// SPDX-License-Identifier: AGPL-3.0-only
import type { DaemonPlace } from "./doctor.js";

/** The daemon's command line on a place, one flag per option the binary takes: the address and port it binds, the
 * token file it checks every auth frame against, the folder its fs and git ops resolve inside and the roots file
 * beside it, the kind that picks its readings modules, the inbox, the manifest and the socket the browser shim
 * posts to, the file it writes its port to where the machine picked one, and `--doors` on a provider's machine.
 * Words, unquoted: the unit and the supervisor quote them in their own language, and a joined computer's manager
 * takes them as they are. */
export function daemonFlags(place: DaemonPlace): string[] {
  return [
    "--host",
    place.bind,
    "--port",
    String(place.port),
    "--token-path",
    place.tokenPath,
    "--root",
    place.root,
    "--roots-path",
    place.rootsPath,
    "--kind",
    place.kind,
    "--inbox",
    place.inbox,
    "--manifest",
    place.manifestPath,
    "--open-socket",
    place.openSocket,
    ...(place.portFile === undefined ? [] : ["--port-file", place.portFile]),
    ...(place.doors === true ? ["--doors"] : []),
  ];
}
