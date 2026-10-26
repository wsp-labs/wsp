// SPDX-License-Identifier: AGPL-3.0-only
// One home for every download's name and for the URL that reaches it: the
// notes print them, the release job uploads under them, and the site links
// them. A second copy anywhere is a link that rots on the next release.
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { bundleEnv, bundleNames, CLASSIC_STABLE_NAMES, classicNames, downloadUrl, GPUI_NAMES, REPO, STABLE_NAMES } from "../../protocol/src/bundles.mjs";

const script = fileURLToPath(new URL("../scripts/bundle-env.mjs", import.meta.url));

describe("what a release's downloads are called", () => {
  it("is one disk image for both mac chips, the zip the app updates itself from, and one AppImage for linux, each carrying the version", () => {
    expect(bundleNames("0.1.5")).toEqual({ mac: "wsp-0.1.5-mac.dmg", macZip: "wsp-0.1.5-mac.zip", appImage: "wsp-0.1.5.AppImage" });
    expect(bundleNames("1.0.0-rc.1")).toEqual({ mac: "wsp-1.0.0-rc.1-mac.dmg", macZip: "wsp-1.0.0-rc.1-mac.zip", appImage: "wsp-1.0.0-rc.1.AppImage" });
  });

  it("has a copy of each under a name no version moves, of the same kind of file", () => {
    expect(STABLE_NAMES).toEqual({ mac: "wsp-mac.dmg", appImage: "wsp-linux.AppImage" });
    expect(CLASSIC_STABLE_NAMES).toEqual({ mac: "wsp-classic-mac.dmg", appImage: "wsp-classic-linux.AppImage" });
    for (const [stable, versioned] of [[STABLE_NAMES, bundleNames("0.1.5")], [CLASSIC_STABLE_NAMES, classicNames("0.1.5")]] as const) {
      for (const kind of ["mac", "appImage"] as const) {
        const suffix = (name: string) => name.slice(name.lastIndexOf("."));
        expect(suffix(stable[kind])).toBe(suffix(versioned[kind]));
        expect(stable[kind]).not.toContain("0.1.5");
      }
    }
  });

  it("names the GPUI app under the names today's release has always used, which its updater reads off GPUI_NAMES, and the classic app apart", () => {
    expect(GPUI_NAMES).toEqual({ mac: "wsp-{version}-mac.dmg", macZip: "wsp-{version}-mac.zip", appImage: "wsp-{version}.AppImage" });
    expect(bundleNames("0.1.5")).toEqual({ mac: "wsp-0.1.5-mac.dmg", macZip: "wsp-0.1.5-mac.zip", appImage: "wsp-0.1.5.AppImage" });
    // The classic app's downloads stand apart from the GPUI app's, each saying classic.
    expect(classicNames("0.1.5")).toEqual({ mac: "wsp-classic-0.1.5-mac.dmg", macZip: "wsp-classic-0.1.5-mac.zip", appImage: "wsp-classic-0.1.5.AppImage" });
    const gpui = new Set([...Object.values(bundleNames("0.1.5")), ...Object.values(STABLE_NAMES)]);
    for (const name of [...Object.values(classicNames("0.1.5")), ...Object.values(CLASSIC_STABLE_NAMES)]) {
      expect(name).toContain("classic");
      expect(gpui.has(name)).toBe(false);
    }
  });
});

describe("where a download is reached", () => {
  it("is the newest release's copy, which is the only link that survives a release", () => {
    expect(downloadUrl(STABLE_NAMES.mac)).toBe(`${REPO}/releases/latest/download/wsp-mac.dmg`);
    expect(downloadUrl(STABLE_NAMES.appImage)).toBe(`${REPO}/releases/latest/download/wsp-linux.AppImage`);
    expect(REPO).toBe("https://github.com/wsp-labs/wsp");
  });
});

describe("the names the release job reads", () => {
  it("are the ten assets it uploads, the GPUI app's then the classic app's, as the assignments a step appends to its environment", () => {
    expect(bundleEnv("0.1.5")).toBe(
      "MAC_DMG=wsp-0.1.5-mac.dmg\nMAC_STABLE=wsp-mac.dmg\nMAC_ZIP=wsp-0.1.5-mac.zip\nAPPIMAGE=wsp-0.1.5.AppImage\nAPPIMAGE_STABLE=wsp-linux.AppImage\n" +
        "CLASSIC_MAC_DMG=wsp-classic-0.1.5-mac.dmg\nCLASSIC_MAC_STABLE=wsp-classic-mac.dmg\nCLASSIC_MAC_ZIP=wsp-classic-0.1.5-mac.zip\n" +
        "CLASSIC_APPIMAGE=wsp-classic-0.1.5.AppImage\nCLASSIC_APPIMAGE_STABLE=wsp-classic-linux.AppImage\n",
    );
  });

  it("come out of the script the workflow runs, for the version it is handed", () => {
    expect(execFileSync("node", [script, "0.1.5"], { encoding: "utf8" })).toBe(bundleEnv("0.1.5"));
  });
});
