// SPDX-License-Identifier: AGPL-3.0-only
// The web's sidebar update card alone, in the stage `?state=` names (get, downloading, restart), on a 256 px sidebar
// in Paper, with the store and the shell's bridge set as a desktop app on its own host sets them.
import type { DesktopBridge } from "@wsp/protocol";
import { createRoot } from "react-dom/client";
import "../../../../web/src/index.css";
import "../../../../web/src/themes/index.js";
import { useStore } from "../../../../web/src/protocol/store.js";
import { useUpdateDownload } from "../../../../web/src/shell/update.js";
import { UpdateCard } from "../../../../web/src/sidebar/UpdateCard.js";

const RELEASE = "99.0.0";
const state = new URLSearchParams(location.search).get("state") ?? "get";
const inPlace = state !== "get";
const bridge: Partial<DesktopBridge> = {
  version: "0.3.0",
  getBundle: () => new Promise(() => undefined),
  quitAndOpen: () => new Promise(() => undefined),
  discardUpdate: () => Promise.resolve(),
  bundleHover: "hover",
  updatesInPlace: inPlace,
};
(window as unknown as { wsp: Partial<DesktopBridge> }).wsp = bridge;

const running = (i: number) => ({ id: `r-busy${i}`, threadId: `busy${i}`, workspaceId: "w0", harness: "claude", status: "running", prompt: "Working", startedAt: Date.now() });
useStore.setState({
  release: { state: "read", latest: { version: RELEASE, tag: `v${RELEASE}`, url: "https://example.invalid/notes", publishedAt: "2026-10-20T00:00:00Z" } },
  preferencesRead: true,
  preferences: {},
  sessions: { w0: (state === "restart" ? [0, 1] : []).map(running) },
} as never);
useUpdateDownload.setState({ download: state === "get" ? null : { version: RELEASE, phase: state === "downloading" ? "downloading" : "kept" } });

document.documentElement.dataset.theme = "paper";
createRoot(document.getElementById("root")!).render(
  <div data-app-sidebar="" className="bg-sidebar text-sidebar-foreground" style={{ width: 256, padding: 8 }}>
    <UpdateCard />
  </div>,
);
