// SPDX-License-Identifier: AGPL-3.0-only
//! Whether the service host follows this app onto its files now (`replaceHost` in `host-lifecycle.ts`). A turn leads
//! a process group of its own, so a restart leaves it running and the next host picks it up; still, a restart nobody
//! asked for goes only while no turn runs, and while any runs, or where the host would not say, the person is asked
//! first and nothing is stopped until they answer. A restart the person asked for (Restart on the update card, which
//! says the running threads carry on) goes at once.

/// What to do about a host this app's files have moved past.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Follow {
    Restart,
    Ask,
}

/// A restart nobody asked for, by `working`: the turns running on this computer, or nothing where the host would not
/// say. The swap's own restart, which the person asked for, does not come here.
pub fn follow(working: Option<usize>) -> Follow {
    if working == Some(0) {
        Follow::Restart
    } else {
        Follow::Ask
    }
}

/// `THIS_COMPUTER` in `words/computer.ts`.
const THIS_COMPUTER: &str = "this computer";

/// The question for a host of release `theirs` with `working` turns running on this computer (`replacePrompt`).
pub struct Prompt {
    pub message: String,
    pub detail: String,
    pub restart: String,
    pub quit: &'static str,
}

pub fn replace_prompt(theirs: &str, mine: &str, working: Option<usize>) -> Prompt {
    let running = match working {
        None | Some(0) => String::new(),
        Some(1) => format!("A thread is working on {THIS_COMPUTER}. "),
        Some(n) => format!("{n} threads are working on {THIS_COMPUTER}. "),
    };
    Prompt {
        message: format!("wsp {theirs} is still serving your threads; restart it as {mine}?"),
        detail: format!(
            "{running}Running turns carry on while wsp restarts. This app opens only on wsp {mine}, so Quit leaves wsp {theirs} serving and closes the app."
        ),
        restart: format!("Restart as {mine}"),
        quit: "Quit",
    }
}

/// The release a refused open names as serving, where the refusal is the one an older host here gets
/// (`refusal` in wsp-mcp's `release.rs`: "wsp {theirs} keeps serving {state}: this app opens only on wsp {mine}").
pub fn older_host(message: &str) -> Option<&str> {
    let rest = message.strip_prefix("wsp ")?;
    let (theirs, rest) = rest.split_once(" keeps serving ")?;
    rest.contains(": this app opens only on wsp ").then_some(theirs)
}
