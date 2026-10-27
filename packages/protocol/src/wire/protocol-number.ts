// SPDX-License-Identifier: AGPL-3.0-only
// The protocol number: one integer a host says in its auth reply and a client may bound in its auth request, so a
// client and a host that no longer read each other's frames refuse in a sentence instead of misreading. The Rust
// host's wsp-wire crate mirrors this file's constant and shapes (docs/rust-host-spec/11-finding-a-host.md, Proposal 7).
import { z } from "zod";

/** The number this host speaks. It moves up by one in the change that makes an older client misread the wire: an op
 * removed or renamed, a field removed, renamed or changed in meaning, a refusal's kind changed. An op, an optional
 * field or an event added does not move it. `protocol-shapes.ts` and its test keep that rule. */
export const WIRE_PROTOCOL = 1;

/** The numbers a client speaks, lowest and highest, as the auth request carries them. */
export const ProtocolRange = z.tuple([z.number().int(), z.number().int()]);
export type ProtocolRange = z.infer<typeof ProtocolRange>;

/** What the first frame of a socket answers: the release, the road the host came up on, and the protocol number. */
export const AuthReply = z.object({ version: z.string(), road: z.string().optional(), protocol: z.number().int() });
export type AuthReply = z.infer<typeof AuthReply>;
