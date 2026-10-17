// SPDX-License-Identifier: AGPL-3.0-only
// Whether the cloud is on in this process, read once when it loads: Solari and
// the cloud's top-level roads (fork, snapshot, pause, wake and the rest). Boat
// is registered whatever this reads. The provider table registers Solari on this
// and nothing else; the verbs, the pages and the skill read the same reading,
// since the tool server loads them without the table, whose rows build the
// engine's backends.
import { cloudFromEnv } from "@wsp/protocol";

/** False in a public build, which the bundler fills in where PUBLIC_BUILD=1; never filled in anywhere else. */
declare const __WSP_CLOUD__: boolean | undefined;

/** Whether this build carries Solari and the top-level roads at all: a public build carries Boat alone, so no
 * variable turns the rest on there. */
export const CLOUD_BUILT: boolean = typeof __WSP_CLOUD__ !== "boolean" || __WSP_CLOUD__;

export const CLOUD_ON: boolean = CLOUD_BUILT && cloudFromEnv(process.env);
