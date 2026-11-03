# Electron Default Implementation Plan

**Goal:** Make Electron the default release download while keeping GPUI available as Experimental.

**Architecture:** Preserve versioned asset names and updater selection. Publish Electron under the default stable aliases and GPUI under separate experimental aliases. Prefer the versioned Electron artifact in the install script, retaining the historical fallback. Keep existing classic aliases compatible.

**Tech Stack:** JavaScript, TypeScript, GitHub Actions.

- [x] Change shared release aliases and upload both default and classic Electron aliases.
- [x] Put Electron first in release notes and GPUI under Experimental on the site.
- [x] Verify release naming, workflow, download routes and existing Electron switch/updater behavior with focused tests.
- [x] Report native switch registration and return-path gaps without changing user state.

Verification: 77 release/switch tests and 110 download/site/worker/updater tests passed; site typecheck passed. Installer selection regression failed on the old selector before the fix; patched installer suite passed 18 tests with 5 platform-gated skips. Self-review caught and fixed versioned installer selection after reviewing the stable alias change. The installer recognizes both app kinds. Optional toggle remains deferred: GPUI removes its path registration and switch on launch, has no return control, and both macOS packages use wsp.app with bundle id wsp.desktop. No user state or app processes changed.
