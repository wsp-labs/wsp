// SPDX-License-Identifier: AGPL-3.0-only
//! protocol/src/slate/steps.ts at 65f6b89c7.
/// One handler step kind: what it does, where it runs, what consent it needs and how it is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlateStepModule {
    pub kind: &'static str,
    /// `host`: the batch applies it. `window`: the window that raised the press does, so only a press may hold it.
    pub runs: &'static str,
    pub sig: &'static str,
    pub purpose: &'static str,
    pub consent: &'static str,
    pub example: &'static str,
}

pub const SLATE_STEPS: &[SlateStepModule] = &[
    SlateStepModule {
        kind: "set",
        runs: "host",
        sig: "set($path, value)",
        purpose: "writes a value; later steps read the new value",
        consent: "none",
        example: "set($step, 2)",
    },
    SlateStepModule {
        kind: "toggle",
        runs: "host",
        sig: "toggle($path)",
        purpose: "flips a boolean value",
        consent: "none",
        example: "toggle($showDone)",
    },
    SlateStepModule {
        kind: "start",
        runs: "host",
        sig: "start($run)",
        purpose: "starts a run; does not wait; its end fires <when done>",
        consent: "the person approves the command once",
        example: "start($check)",
    },
    SlateStepModule {
        kind: "cancel",
        runs: "host",
        sig: "cancel($run)",
        purpose: "stops a running run",
        consent: "none",
        example: "cancel($tests)",
    },
    SlateStepModule {
        kind: "send",
        runs: "host",
        sig: "send(\"text\", $path, ...)",
        purpose: "a message to the agent: the text, then slate: {...} with the paths as data",
        consent: "a press is the consent; from a reaction, one approval per slate",
        example: "send(\"Fix the failing check.\", item.name)",
    },
    SlateStepModule {
        kind: "steer",
        runs: "host",
        sig: "steer(\"text\", $path, ...)",
        purpose: "as send, into the running turn",
        consent: "as send",
        example: "steer(\"Look at the failing check first.\")",
    },
    SlateStepModule {
        kind: "queue",
        runs: "host",
        sig: "queue(\"text\", $path, ...)",
        purpose: "as send, after the running turn (a send until the flag lands)",
        consent: "as send",
        example: "queue(\"Then open the pull request.\")",
    },
    SlateStepModule {
        kind: "fill",
        runs: "window",
        sig: "fill(\"text\", $path, ...)",
        purpose: "puts the text in the composer, not sent",
        consent: "none; a press only",
        example: "fill(\"About this branch:\", git.branch)",
    },
    SlateStepModule {
        kind: "open",
        runs: "window",
        sig: "open(url)",
        purpose: "opens an http, https or mailto link",
        consent: "once per domain; a press only",
        example: "open(pr.url)",
    },
    SlateStepModule {
        kind: "copy",
        runs: "window",
        sig: "copy(text)",
        purpose: "copies to the clipboard",
        consent: "none; a press only",
        example: "copy(git.head)",
    },
    SlateStepModule {
        kind: "pane",
        runs: "window",
        sig: "pane(\"kind\")",
        purpose: "opens a right panel tab",
        consent: "none; a press only",
        example: "pane(\"pr\")",
    },
];

pub fn slate_step_module(kind: &str) -> Option<&'static SlateStepModule> {
    SLATE_STEPS.iter().find(|step| step.kind == kind)
}

/// The host a web link opens, which a person allows once per thread; a mailto opens their own mail app and has none.
pub fn slate_link_domain(href: &str) -> Option<String> {
    let url = url::Url::parse(href).ok()?;
    matches!(url.scheme(), "https" | "http").then(|| url.host_str().unwrap_or_default().to_lowercase())
}

/// The approval a person's "Always for this domain" is kept under, beside the slate's command approvals.
pub fn slate_domain_key(domain: &str) -> String {
    format!("domain:{domain}")
}
