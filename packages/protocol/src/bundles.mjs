// SPDX-License-Identifier: AGPL-3.0-only
// Every name a download carries and every URL that reaches one, in one place:
// the release notes, the release job and the site all read them from here, so
// a rename touches this file alone. Nothing here imports node, because the
// site bundles it for a browser.

export const REPO = "https://github.com/wsp-labs/wsp";

/** What a release tag looks like, v1.2.3 carrying 1.2.3: the one rule for what the release workflow answers to
 * and for what the host takes as the newest release. */
export const RELEASE_TAG = /^v(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)$/;

/** Where every release and its downloads are listed, which is where anything that has no one asset to point at
 * sends a person: the site's footer, and the app's line about a shell older than the host serving it. */
export const RELEASES = `${REPO}/releases`;

/** What the GPUI app's downloads are called, {version} standing for the version: the names today's release has
 * always used, so an installed app of either kind that reads them updates into the GPUI app. The GPUI app reads these
 * three lines out of this file as written, so each stays a key, a colon and one double-quoted string. */
export const GPUI_NAMES = {
  mac: "wsp-{version}-mac.dmg",
  macZip: "wsp-{version}-mac.zip",
  appImage: "wsp-{version}.AppImage",
};

/** What the classic app's downloads are called, the Electron app the release lists second. */
export const CLASSIC_NAMES = {
  mac: "wsp-classic-{version}-mac.dmg",
  macZip: "wsp-classic-{version}-mac.zip",
  appImage: "wsp-classic-{version}.AppImage",
};

const filled = (names, version) => ({
  mac: names.mac.replace("{version}", version),
  macZip: names.macZip.replace("{version}", version),
  appImage: names.appImage.replace("{version}", version),
});

/** What each of the GPUI app's downloads on the release page is called for one version, which is the name the notes
 * print first. The zip is what an installed mac app replaces itself from, so the notes leave it out. */
export function bundleNames(version) {
  return filled(GPUI_NAMES, version);
}

/** The same for the classic app, which the notes print second. */
export function classicNames(version) {
  return filled(CLASSIC_NAMES, version);
}

/** The copies the release job uploads beside the versioned ones, under names no version moves. GitHub serves the
 * newest release's copy of each at releases/latest/download, which is the only download link the site can hold. */
export const STABLE_NAMES = { mac: "wsp-mac.dmg", appImage: "wsp-linux.AppImage" };
export const CLASSIC_STABLE_NAMES = { mac: "wsp-classic-mac.dmg", appImage: "wsp-classic-linux.AppImage" };

/** The last release that carried the classic app under STABLE_NAMES, where the site sends a classic download while the
 * newest release has no copy under CLASSIC_STABLE_NAMES. */
export const CLASSIC_FALLBACK_TAG = "v0.3.8";

/** Where GitHub serves that asset of whichever release is newest. */
export function downloadUrl(asset) {
  return `${REPO}/releases/latest/download/${asset}`;
}

/** The names one release's jobs work with, as the assignments each writes into its own environment: the GPUI app's,
 * then the classic app's. */
export function bundleEnv(version) {
  const gpui = bundleNames(version);
  const classic = classicNames(version);
  const lines = [
    ["MAC_DMG", gpui.mac],
    ["MAC_STABLE", STABLE_NAMES.mac],
    ["MAC_ZIP", gpui.macZip],
    ["APPIMAGE", gpui.appImage],
    ["APPIMAGE_STABLE", STABLE_NAMES.appImage],
    ["CLASSIC_MAC_DMG", classic.mac],
    ["CLASSIC_MAC_STABLE", CLASSIC_STABLE_NAMES.mac],
    ["CLASSIC_MAC_ZIP", classic.macZip],
    ["CLASSIC_APPIMAGE", classic.appImage],
    ["CLASSIC_APPIMAGE_STABLE", CLASSIC_STABLE_NAMES.appImage],
  ];
  return `${lines.map(([name, value]) => `${name}=${value}`).join("\n")}\n`;
}
