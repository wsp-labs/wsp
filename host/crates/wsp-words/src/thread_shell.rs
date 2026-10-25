// SPDX-License-Identifier: AGPL-3.0-only
//! The startup files that put a thread's own folder first on PATH in every shell a turn starts. Port of
//! packages/protocol/src/thread-shell.ts at the cut (383708c78). See spec/shell_and_text.md.

use indexmap::IndexMap;
use wsp_wire::types::shell_quote::shell_quote;

// The files zsh starts from where ZDOTDIR names that folder, in the order a login shell reads them.
// thread-shell.ts:5
// private in the TS: port it as a private helper
// const ZSH_FILES = [".zshenv", ".zprofile", ".zshrc", ".zlogin"] as const;
const ZSH_FILES: [&str; 4] = [".zshenv", ".zprofile", ".zshrc", ".zlogin"];

/// What bash reads where BASH_ENV names it: after the profile on bash -lc, and alone on bash -c.
// thread-shell.ts:7
const BASH_ENV_FILE: &str = ".bash_env";

/// The variables that make a turn's shells start from the folder's own files, on this computer and on a box alike.
// thread-shell.ts:10
// export const threadShellVars = (dir: string): { ZDOTDIR: string; BASH_ENV: string } => ({ ZDOTDIR: dir, BASH_ENV: `${dir}/${BASH_ENV_FILE}` });
/// `{ ZDOTDIR, BASH_ENV }`, in that order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellVars {
    pub zdotdir: String,
    pub bash_env: String,
}

pub fn thread_shell_vars(dir: &str) -> ShellVars {
    ShellVars { zdotdir: dir.to_string(), bash_env: format!("{dir}/{BASH_ENV_FILE}") }
}

/// The startup files, by name, that put `dir` back first on PATH. A login shell re-reads the person's profile and
/// rebuilds PATH, which puts the folder behind any wsp the profile adds (Codex runs every command as a login shell, and
/// reached an older install that way, measured 2026-10-05 on codex 0.160.1). So zsh starts from the folder's own files
/// and bash reads its file after the profile: each runs the person's own and then puts the folder back in front. `env`
/// is the environment turns launch under, whose own ZDOTDIR and BASH_ENV the files run in their place.
// thread-shell.ts:17
// export function threadShellFiles(dir: string, env: Readonly<Record<string, string | undefined>> = {}): [string, string][] {
//   const first = shellQuote(`:${dir}:`);
//   const own = env["ZDOTDIR"] === undefined || env["ZDOTDIR"] === "" ? "$HOME" : shellQuote(env["ZDOTDIR"]);
//   // The person's file runs with ZDOTDIR at their own folder, as theirs may read it or move it; zsh reads its next file
//   // out of ZDOTDIR, so it points back here after each. A host started from a thread's shell was launched with a
//   // wrapper folder as ZDOTDIR, this one or another home's: a wrapper never sources its own folder, and keeps the
//   // person's folder another home's wrapper settled on rather than that wrapper's folder.
//   const zsh = ZSH_FILES.map((name): [string, string] => [
//     name,
//     [`ZDOTDIR=\${_wsp_zdotdir:-${own}}`, `[[ $ZDOTDIR -ef ${shellQuote(dir)} ]] && ZDOTDIR=$HOME`, "_wsp_zdotdir=", `[[ -r "$ZDOTDIR/${name}" ]] && source "$ZDOTDIR/${name}"`, "_wsp_zdotdir=${_wsp_zdotdir:-$ZDOTDIR}", `ZDOTDIR=${shellQuote(dir)}`, `[[ :$PATH: == ${first}* ]] || PATH=${shellQuote(dir)}:$PATH`, ""].join("\n"),
//   ]);
//   const bashEnv = env["BASH_ENV"];
//   const bash = [...(bashEnv === undefined || bashEnv === "" ? [] : [`[ -r ${shellQuote(bashEnv)} ] && ! [ ${shellQuote(bashEnv)} -ef ${shellQuote(`${dir}/${BASH_ENV_FILE}`)} ] && . ${shellQuote(bashEnv)}`]), `case :$PATH: in ${first}*) ;; *) PATH=${shellQuote(dir)}:$PATH ;; esac`, ""].join("\n");
//   return [...zsh, [BASH_ENV_FILE, bash]];
// }
/// `env` is the environment the turn starts from, read for ZDOTDIR and BASH_ENV; a missing key and an absent value
/// read alike. Answers (file name, body) pairs: the four zsh files in order, then `.bash_env`.
pub fn thread_shell_files(dir: &str, env: &IndexMap<String, String>) -> Vec<(String, String)> {
    let first = shell_quote(&format!(":{dir}:"));
    let quoted_dir = shell_quote(dir);
    let own = match env.get("ZDOTDIR").filter(|z| !z.is_empty()) {
        Some(z) => shell_quote(z),
        None => "$HOME".to_string(),
    };
    let mut files: Vec<(String, String)> = ZSH_FILES
        .iter()
        .map(|name| {
            let body = [
                format!("ZDOTDIR=${{_wsp_zdotdir:-{own}}}"),
                format!("[[ $ZDOTDIR -ef {quoted_dir} ]] && ZDOTDIR=$HOME"),
                "_wsp_zdotdir=".to_string(),
                format!("[[ -r \"$ZDOTDIR/{name}\" ]] && source \"$ZDOTDIR/{name}\""),
                "_wsp_zdotdir=${_wsp_zdotdir:-$ZDOTDIR}".to_string(),
                format!("ZDOTDIR={quoted_dir}"),
                format!("[[ :$PATH: == {first}* ]] || PATH={quoted_dir}:$PATH"),
                String::new(),
            ]
            .join("\n");
            (name.to_string(), body)
        })
        .collect();
    let mut bash = Vec::new();
    if let Some(bash_env) = env.get("BASH_ENV").filter(|v| !v.is_empty()) {
        let (quoted, own_file) = (shell_quote(bash_env), shell_quote(&format!("{dir}/{BASH_ENV_FILE}")));
        bash.push(format!("[ -r {quoted} ] && ! [ {quoted} -ef {own_file} ] && . {quoted}"));
    }
    bash.push(format!("case :$PATH: in {first}*) ;; *) PATH={quoted_dir}:$PATH ;; esac"));
    bash.push(String::new());
    files.push((BASH_ENV_FILE.to_string(), bash.join("\n")));
    files
}
