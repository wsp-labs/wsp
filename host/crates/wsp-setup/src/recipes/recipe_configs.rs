// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `recipe-configs.ts`: the git and shell configs a recipe carries to a computer, read with their cuts made.
use wsp_catalog::js_regex::JsRegex;

/// `ConfigFiles` (`recipe-configs.ts:15`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConfigFiles {
    Git,
    Shell,
}

/// `CONFIG_PATHS.git` (`recipe-configs.ts:19`).
pub const CONFIG_PATHS_GIT: &[&str] = &[".gitconfig", ".config/git/config", ".config/git/ignore", ".config/git/attributes"];

/// `CONFIG_PATHS.shell` (`recipe-configs.ts:19`).
pub const CONFIG_PATHS_SHELL: &[&str] = &[
    ".zshrc",
    ".zshenv",
    ".zprofile",
    ".zlogin",
    ".zsh_aliases",
    ".zsh_plugins.txt",
    ".p10k.zsh",
    ".tmux.conf",
    ".config/starship.toml",
    ".config/fish",
    ".config/tmux",
    ".config/zellij",
    ".config/direnv",
    ".config/oh-my-posh",
    ".oh-my-zsh/custom",
];

/// `FISH_VARIABLES` (`recipe-configs.ts:42`).
pub const FISH_VARIABLES: &str = "fish_variables";

/// `GH_CREDENTIAL_HELPER` (`recipe-configs.ts:78`).
pub const GH_CREDENTIAL_HELPER: &str = "!gh auth git-credential";

/// `GitCuts` (`recipe-configs.ts:84`).
#[derive(Clone, Debug, PartialEq)]
pub struct GitCuts {
    pub sections: JsRegex,
    pub runs: bool,
    pub helper: GitHelper,
}

/// `GitCuts["helper"]` (`recipe-configs.ts:86`), `"always"` or `"replacing"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GitHelper {
    Always,
    Replacing,
}

/// `CLOUD_GIT_CUTS` (`recipe-configs.ts:93`), computed by a constructor because it holds a regex.
pub fn cloud_git_cuts() -> GitCuts {
    todo!()
}

/// `shellScript` (`recipe-configs.ts:97`).
pub fn shell_script(_path: &str, _rc: bool) -> bool {
    todo!()
}

/// The inline return of `shellCut` (`recipe-configs.ts:105`).
#[derive(Clone, Debug, PartialEq)]
pub struct ShellCut {
    pub names: Vec<String>,
    pub held: Vec<String>,
    pub carried: String,
}

/// `shellCut` (`recipe-configs.ts:105`).
pub fn shell_cut(_text: &str, _script: bool) -> ShellCut {
    todo!()
}

/// `gitCut` (`recipe-configs.ts:112`).
pub fn git_cut(_text: &str, _main: bool, _cuts: &GitCuts, _names: &mut Vec<String>) -> String {
    todo!()
}

/// `ConfigText` (`recipe-configs.ts:169`).
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigText {
    pub rel: String,
    pub text: String,
    pub cut: Vec<String>,
}

/// `configTexts` (`recipe-configs.ts:188`).
pub fn config_texts(_id: ConfigFiles, _home: &str) -> Vec<ConfigText> {
    todo!()
}

/// `configDigest` (`recipe-configs.ts:203`).
pub fn config_digest(_texts: &[ConfigText]) -> String {
    todo!()
}
