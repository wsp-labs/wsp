// SPDX-License-Identifier: AGPL-3.0-only
// The install: the install line first on a Mac or Linux, since a file curl writes meets no security prompt; the app's
// download beside it, through the first-open guide on a Mac while the app is unsigned; the waitlist on Windows.
import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../src/App";
import { downloadUrl, STABLE_NAMES } from "../../../packages/protocol/src/bundles.mjs";
import { INSTALL, RELEASES } from "../src/links";
import { platformOf } from "../src/downloads";
import { MAC_GUIDE } from "../src/components/download";
import { MAC_STEPS } from "../src/pages/download-mac";

/** What a browser says it is running on, from the shapes that actually reach this page. */
const AGENTS = {
  "Safari on an M-series Mac":
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15",
  "Firefox on Linux": "Mozilla/5.0 (X11; Linux x86_64; rv:129.0) Gecko/20100101 Firefox/129.0",
  "Chrome on a Chromebook": "Mozilla/5.0 (X11; CrOS x86_64 14541.0.0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36",
  "Chrome on Windows": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36",
  "Safari on an iPhone":
    "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1",
  "Chrome on Android": "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Mobile Safari/537.36",
} as const;

const FIRST: Record<keyof typeof AGENTS, "mac" | "linux" | "windows"> = {
  "Safari on an M-series Mac": "mac",
  "Firefox on Linux": "linux",
  "Chrome on a Chromebook": "linux",
  "Chrome on Windows": "windows",
  "Safari on an iPhone": "mac",
  "Chrome on Android": "mac",
};

const HREF = { mac: downloadUrl(STABLE_NAMES.mac), linux: downloadUrl(STABLE_NAMES.appImage) } as const;

const realAgent = navigator.userAgent;
function browsing(userAgent: string): void {
  Object.defineProperty(navigator, "userAgent", { value: userAgent, configurable: true });
}
afterEach(() => browsing(realAgent));

describe("which download comes first", () => {
  for (const [browser, agent] of Object.entries(AGENTS)) {
    it(`is ${FIRST[browser as keyof typeof AGENTS]} for ${browser}`, () => {
      expect(platformOf(agent)).toBe(FIRST[browser as keyof typeof AGENTS]);
    });
  }

  it("reads the platform and never the chip, which Safari on Apple silicon reports as Intel", () => {
    expect(AGENTS["Safari on an M-series Mac"]).toContain("Intel");
    expect(platformOf(AGENTS["Safari on an M-series Mac"])).toBe("mac");
  });
});

describe("the install on the page", () => {
  it("puts the install line first on a Mac, and sends the app's download through the first-open guide", () => {
    browsing(AGENTS["Safari on an M-series Mac"]);
    render(<App />);
    expect(screen.getAllByRole("button", { name: `Copy ${INSTALL}` })).toHaveLength(2);
    expect(screen.getAllByRole("link", { name: "or download the app" }).map(a => a.getAttribute("href"))).toEqual([MAC_GUIDE, MAC_GUIDE]);
    expect(screen.getByRole("link", { name: "Download" }).getAttribute("href")).toBe(MAC_GUIDE);
    expect(document.querySelector(`a[href="${HREF.mac}"]`)).toBeNull();
  });

  it("puts the install line first on Linux, with the AppImage beside it, the newest release's asset and no releases page", () => {
    for (const href of Object.values(HREF)) {
      expect(href.startsWith(`${RELEASES}/latest/download/`)).toBe(true);
      expect(href).not.toMatch(/\d+\.\d+\.\d+/);
    }
    browsing(AGENTS["Firefox on Linux"]);
    render(<App />);
    expect(screen.getAllByRole("button", { name: `Copy ${INSTALL}` })).toHaveLength(2);
    expect(screen.getAllByRole("link", { name: "or download the AppImage" }).map(a => a.getAttribute("href"))).toEqual([HREF.linux, HREF.linux]);
    expect(screen.getByRole("link", { name: "Download" }).getAttribute("href")).toBe(HREF.linux);
  });

  it("gives a Windows visitor the waitlist and no download that would not run", () => {
    browsing(AGENTS["Chrome on Windows"]);
    render(<App />);
    expect(screen.queryAllByRole("link", { name: /^or download/ })).toHaveLength(0);
    expect(screen.queryAllByRole("button", { name: `Copy ${INSTALL}` })).toHaveLength(0);
    expect(screen.getAllByRole("button", { name: /Join the waitlist/ })).toHaveLength(2);
  });
});

describe("the Mac's first-open guide", () => {
  it("starts the download, walks the four steps, and offers the install line instead", () => {
    render(<App path={MAC_GUIDE} />);
    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("Your download has started.");
    expect(screen.getByRole("link", { name: "Download again" }).getAttribute("href")).toBe(HREF.mac);
    expect(screen.getAllByRole("listitem").filter(li => li.closest("main")).length).toBe(MAC_STEPS.length);
    expect(screen.getByText(/Privacy & Security/)).toBeTruthy();
    expect(screen.getByRole("button", { name: `Copy ${INSTALL}` })).toBeTruthy();
  });
});
