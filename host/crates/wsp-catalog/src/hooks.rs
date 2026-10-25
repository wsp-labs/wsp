// SPDX-License-Identifier: AGPL-3.0-only
//! How an agent's settings name the hook scripts it runs, so a copy brings each script or takes the hook out. Port of
//! packages/catalog/src/hooks.ts and codex-hooks.ts at a3b870a6e; the spec is spec/hooks.md.
use serde::Serialize;

use crate::pending::HostError;

/// Where a script lands on the guest, absolute; nothing when it cannot travel (hooks.ts:11).
pub type HookPlacer<'a> = &'a dyn Fn(&str) -> Option<String>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Carried {
    pub from: String,
    pub to: String,
}

/// hooks.ts:13.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarriedHooks {
    pub text: String,
    pub carried: Vec<Carried>,
    pub left: Vec<String>,
}

pub const CLAUDE_SETTINGS_FILE: &str = "~/.claude/settings.json";
pub const CODEX_CONFIG_FILE: &str = "~/.codex/config.toml";

/// One module per agent (hooks.ts:132, codex-hooks.ts:221).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookCarry {
    Claude,
    Codex,
}

impl HookCarry {
    pub fn file(self) -> &'static str {
        todo!()
    }

    /// The settings as the machine gets them: the input byte for byte where nothing moved (hooks.ts:134,
    /// codex-hooks.ts:223). Claude Code's edit may refuse as `edit_json` refuses.
    pub fn carry(self, text: &str, home: &str, place: HookPlacer<'_>) -> Result<CarriedHooks, HostError> {
        todo!()
    }
}

/// An absolute path under /bin/, /usr/bin/, /usr/local/bin/ or /usr/sbin/ with a name after it (hooks.ts:39).
pub fn on_machine(abs: &str) -> bool {
    todo!()
}

/// A word's path, `~/`, `$HOME/` and `${HOME}/` expanded; nothing for a word that is not a path (hooks.ts:42).
pub fn path_of(word: &str, home: &str) -> Option<String> {
    todo!()
}

/// hooks.ts:82.
pub fn runs_a_file(word: &str, home: &str) -> bool {
    todo!()
}

/// hooks.ts:88.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlacedWord {
    Word { word: String, carried: Option<Carried> },
    Left(String),
}

/// hooks.ts:90.
pub fn place_word(word: &str, home: &str, place: HookPlacer<'_>) -> PlacedWord {
    todo!()
}

/// Every path under home a settings file's hooks run, once each (hooks.ts:98).
pub fn hook_files(hooks: HookCarry, text: &str, home: &str) -> Result<Vec<String>, HostError> {
    todo!()
}
