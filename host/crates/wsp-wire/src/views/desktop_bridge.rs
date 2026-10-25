// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/desktop-bridge.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

/// TS: packages/protocol/src/views/desktop-bridge.ts:20 at a3b870a6e.
/// A JavaScript RegExp, flags "": the pattern source as the TS writes it.
pub const BOOT_SCRIPT: &str = "<script>window\\.__WSP__ = ([^<]*);<\\/script>";

/// TS: packages/protocol/src/views/desktop-bridge.ts:111 at a3b870a6e.
pub const DESKTOP_MAC_CLASS: &str = "desktop-mac";

/// TS: packages/protocol/src/views/desktop-bridge.ts:115 at a3b870a6e.
pub const DESKTOP_WCO_CLASS: &str = "wco";

/// TS: packages/protocol/src/views/desktop-bridge.ts:131 at a3b870a6e.
pub const BROWSER_PARTITION: &str = "persist:wsp-browser";

// ---- Hand-written below: the functions of the unit.

use serde::{Deserialize, Serialize};

// views/desktop-bridge.ts:37 at a3b870a6e
// export interface BootPayload {
//   /** The sha256 of the host's own token, hex, inlined only on the loopback page: the desktop shell compares it to the
//    * digest of the token file beside the state and sends nothing, so a page a squatter serves on the lock's port
//    * cannot learn the token by being read. The token itself is never in any page: a browser here dials with a
//    * device token wsp init or wsp host pair let it in with, and the shell hands its page the file's over the bridge. */
//   tokenHash?: string;
//   /** The path on this page's own origin the runtime WebSocket answers on, which is the one road a client reaching
//    * the host through an ssh forward or a tunnel hostname has. */
//   wsPath: string;
//   /** True when this page was served on the computer the host runs on, which is the page's one reading of being at
//    * home: false is a host on another computer, whose page pairs for a device token of its own. */
//   paired: boolean;
//   /** The release this host is, which is the release this page is: a desktop shell attached to a host it did not
//    * start reads it to tell whether the two halves were built apart. */
//   version: string;
//   /** The family the person's terminal draws with, when the saved recipe ticks its row; the terminal pane defaults to it. */
//   terminalFont?: string;
//   /** The state file this host serves; the page keeps what it remembers (the workspace open last) under it. A page
//    * served beyond loopback carries none, so every stranger's page remembers under one empty slot. */
//   statePath?: string;
//   /** Why this host sends no usage counts whatever the switch says, so the Privacy switch shows off with the reason and
//    * the first run says nothing about them. Absent where the switch decides. */
//   productUsageOff?: ProductUsageOff;
// }
/// What the host writes into the page's one inline script as window.__WSP__. Keys in the interface's order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootPayload {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_hash: Option<String>,
    pub ws_path: String,
    pub paired: bool,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_font: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product_usage_off: Option<ProductUsageOff>,
}

// views/desktop-bridge.ts:63 at a3b870a6e
// export type ProductUsageOff = "build" | "env";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProductUsageOff {
    Build,
    Env,
}

// views/desktop-bridge.ts:68 at a3b870a6e
// export interface ContextMenuItem {
//   id: string;
//   label: string;
//   group: string;
//   enabled: boolean;
//   refusal?: string;
//   /** What a row that can run says on hover, where the label leaves something out a person would want before pressing
//    * it. The two are one slot, read refusal first: a row is either dimmed with a reason or live with a word about
//    * what it does, never both, which is the reading every button in the app already makes. */
//   hint?: string;
//   shortcut?: string;
//   accelerator?: string;
//   destructive?: boolean;
//   /** A row that marks a state, drawn with a check when true; a row with no mark at all leaves this out. */
//   checked?: boolean;
// }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextMenuItem {
    pub id: String,
    pub label: String,
    pub group: String,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accelerator: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destructive: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
}

// views/desktop-bridge.ts:89 at a3b870a6e
// export interface HostListing {
//   alias: string;
//   url: string;
// }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostListing {
    pub alias: String,
    pub url: String,
}

// views/desktop-bridge.ts:96 at a3b870a6e
// export interface HostsView {
//   here: string;
//   current: string | null;
//   hosts: HostListing[];
// }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostsView {
    pub here: String,
    /// null where the window is on no listed host.
    pub current: Option<String>,
    pub hosts: Vec<HostListing>,
}
