// SPDX-License-Identifier: AGPL-3.0-only
// What bundles.mjs hands a TypeScript reader. The module itself stays plain
// JavaScript because the release job runs it with no build behind it; this is
// how the site and the app read it without opening their compilers to JS.
type Names = { mac: string; macZip: string; appImage: string };
/** The GPUI app's download names, {version} standing for the version; the release lists them first. */
export const GPUI_NAMES: Names;
/** The classic app's download names, {version} standing for the version; the release lists them second. */
export const CLASSIC_NAMES: Names;
/** What each of the GPUI app's downloads on the release page is called for one version, which is the name the notes
 * print; the zip is what an installed mac app replaces itself from. */
export function bundleNames(version: string): Names;
/** The same for the classic app. */
export function classicNames(version: string): Names;
/** The copies the release job uploads beside the versioned ones, under names no version moves. */
export const STABLE_NAMES: { mac: string; appImage: string };
export const CLASSIC_STABLE_NAMES: { mac: string; appImage: string };
/** The last release that carried the classic app under STABLE_NAMES. */
export const CLASSIC_FALLBACK_TAG: string;
/** Where GitHub serves that asset of whichever release is newest. */
export function downloadUrl(asset: string): string;
/** The names one release's jobs work with, as the assignments it writes into its own environment. */
export function bundleEnv(version: string): string;
export const REPO: string;
/** What a release tag looks like, v1.2.3 carrying 1.2.3. */
export const RELEASE_TAG: RegExp;
/** Where every release and its downloads are listed. */
export const RELEASES: string;
