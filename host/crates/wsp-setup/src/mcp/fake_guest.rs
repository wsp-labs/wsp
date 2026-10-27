// SPDX-License-Identifier: AGPL-3.0-only
//! `fake-guest.ts`: the guest a stand-in provider's machines get when a harness names a folder for them, with this computer's daemon in it.
use std::rc::Rc;

use wsp_engine::fake_backend::FakeGuest;

/// `standInTokenPath` (`fake-guest.ts:51`).
pub fn stand_in_token_path(_root: &str, _machine_id: &str, _of: u32) -> String {
    todo!()
}

/// `guestPath` (`fake-guest.ts:119`).
pub fn guest_path(_at: &str, _path: &str) -> String {
    todo!()
}

/// `inGuestRoot` (`fake-guest.ts:127`).
pub fn in_guest_root(_at: &str, _cmd: &str) -> String {
    todo!()
}

/// `fakeNoPortLine` (`fake-guest.ts:148`).
pub fn fake_no_port_line(_machine_id: &str, _port: u16) -> String {
    todo!()
}

/// `fakeGuestAt` (`fake-guest.ts:153`).
pub fn fake_guest_at(_root: &str) -> Rc<dyn FakeGuest> {
    todo!()
}

/// `closeStandInGuests` (`fake-guest.ts:206`), which skips a daemon that never started rather than failing.
pub async fn close_stand_in_guests() {
    todo!()
}
