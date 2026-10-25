// SPDX-License-Identifier: AGPL-3.0-only
//! Every op the web app sends the host, as a typed call, grouped by the screen that sends it. Each group names an op's
//! request, which record.mjs generates, and its reply: a recorded schema where the protocol has one, else a wrapper
//! written here for a reply the protocol gives only in a comment.

/// Each op of a group as `"op.name" => <Op>Request => Reply`, and the group's list of them for the parity test.
macro_rules! ops {
    ($($name:literal => $request:ident => $reply:ty),* $(,)?) => {
        $(impl crate::Op for crate::protocol::$request {
            const NAME: &'static str = $name;
            type Reply = $reply;
        })*
        pub(crate) const OPS: &[(&str, &str)] = &[$(($name, stringify!($request))),*];
    };
}

pub mod activity;
pub mod dialogs;
pub mod pull_request;
pub mod settings;
pub mod settings_agents;
pub mod sidebar;
pub mod slates;
pub mod terminal;
pub mod thread;
pub mod usage;

/// Every op with a call, beside the name of its request type.
pub fn all() -> impl Iterator<Item = (&'static str, &'static str)> {
    [
        activity::OPS,
        dialogs::OPS,
        pull_request::OPS,
        settings::OPS,
        settings_agents::OPS,
        sidebar::OPS,
        slates::OPS,
        terminal::OPS,
        thread::OPS,
        usage::OPS,
    ]
    .into_iter()
    .flatten()
    .copied()
}
