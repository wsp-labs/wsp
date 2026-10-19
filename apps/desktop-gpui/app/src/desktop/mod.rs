// SPDX-License-Identifier: AGPL-3.0-only
//! What the desktop shell does around the window (`apps/desktop/src/main.ts`): the switch between the two apps, links,
//! zoom, the Linux frame, and the host feed behind the badge, the tray, notifications, keep-awake and the Quit box.

pub mod frame;
pub mod links;
pub mod switch;
pub mod zoom;

pub mod awake;
pub mod feed;
pub mod mac;
pub mod notify;
pub mod quit;
pub mod tray;
