// SPDX-License-Identifier: AGPL-3.0-only
// The policy a frame's page is drawn under, alone in its module so the GPUI app's bundle of the seal takes it and
// nothing else of the protocol.
/** What a frame's page may do, set on it before its own first byte: inline script and style, pictures it carries as
 * data: or blob:, and nothing that loads or sends. Trusted Types with only the two policies the frame makes first
 * keeps the page from writing a nested page, the one road to a realm the frame did not seal. */
export const SLATE_FRAME_POLICY =
  "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data: blob:; font-src data:; media-src data: blob:; form-action 'none'; base-uri 'none'; frame-src 'none'; worker-src 'none'; require-trusted-types-for 'script'; trusted-types default wsp-raw";
