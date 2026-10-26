// SPDX-License-Identifier: AGPL-3.0-only
// The #2449 review: a variable that is not UTF-8 in the process's environment. Its own test binary, since the
// variable it sets would reach every test that reads the environment in the same process.
#![allow(clippy::result_large_err, clippy::type_complexity, clippy::too_many_arguments, clippy::large_enum_variant, dead_code)]

#[path = "common/mod.rs"]
mod pty;

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::sync::Arc;

use futures::FutureExt;
use pty::{first_pty, terminal, tick, FakeLink, Link};
use wsp_cli::signin_relay::{relay_pty, RelayOptions};

/// node reads such a variable lossily into `process.env` and the relay shows the link as always
/// (signin-relay.ts:216-222). The Rust offer line paints through `std::env::vars()` (signin_relay.rs:797), which
/// panics on the first variable that is not UTF-8, as an `OLDPWD` left in a folder with a latin-1 name is.
#[tokio::test]
async fn a_variable_that_is_not_utf8_does_not_end_a_sign_in_that_shows_a_link() {
    std::env::set_var(OsStr::from_bytes(b"WSP_REVIEW_LATIN1"), OsStr::from_bytes(b"caf\xe9"));
    let link = FakeLink::new();
    let (t, rt) = terminal(false, 120, 40);
    let o = RelayOptions {
        link: Arc::new(Link(link.clone())),
        command: None,
        open: Arc::new(|_| async { true }.boxed()),
        callback_url: None,
        on_consent: None,
        env: None,
        timeout_ms: 60_000,
        flush_ms: None,
        now: None,
    };
    let mut rt = rt;
    let run = tokio::spawn(async move { relay_pty(o, &mut rt).await });
    let p = first_pty(&link).await;
    tick().await;
    let shown = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| link.data(&p.id, "visit: https://example.test/login\r\n")));
    assert!(shown.is_ok(), "showing the link panicked");
    assert!(t.text().contains("o opens it on this computer"));
    link.exit(&p.id, 0);
    run.await.unwrap().unwrap();
}
