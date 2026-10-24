// SPDX-License-Identifier: AGPL-3.0-only
// Wave 1's states for baseline.mjs, one per state spec.md and more.md give a baseline of its own, in spec order.
import frameSettingsUsage from "./frame-settings-usage.mjs";
import sidebarThreadComposer, { setup as sidebarSetup } from "./sidebar-thread-composer.mjs";
import wholeWindow from "./whole-window.mjs";

export const setup = sidebarSetup;

export default [...frameSettingsUsage, ...sidebarThreadComposer, ...wholeWindow];
