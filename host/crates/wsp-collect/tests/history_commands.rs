// SPDX-License-Identifier: AGPL-3.0-only
//! The command parser that turns a shell line into the names of what it ran. Port of the `command names` block of
//! packages/collect/test/history.test.ts.
use wsp_collect::history::commands::{command_names, install_names, split_commands, without_heredocs, Install};

fn names(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

fn installs(rows: &[(&str, &str)]) -> Vec<Install> {
    rows.iter().map(|(via, name)| Install { via: via.to_string(), name: name.to_string() }).collect()
}

#[test]
fn takes_the_first_word_of_each_command_across_pipes_ands_semicolons_and_subshells() {
    assert_eq!(
        command_names("git log --oneline | head -5 && gh pr view; (cd x && pnpm test) || echo no"),
        names(&["git", "head", "gh", "pnpm"])
    );
    assert_eq!(command_names("for f in *.ts; do wc -l \"$f\"; done"), names(&["wc"]));
    assert_eq!(command_names("VAR=1 sudo -u me env FOO=bar /usr/local/bin/python3 -c 1"), names(&["python3"]));
    assert_eq!(command_names("time nohup node server.js &"), names(&["node"]));
    // A comment ends the line's commands; a quoted # is text; a substitution inside double quotes still runs its command.
    assert_eq!(command_names("ls # list\ngit commit -m \"fix #12\"\ncat \"$(mktemp)\""), names(&["ls", "git", "cat", "mktemp"]));
    assert_eq!(command_names("echo $(git rev-parse HEAD) > `which out`"), names(&["git"]));
    assert_eq!(command_names("2>/dev/null ls; > out.txt cat in; curl -s x >/dev/null 2>&1"), names(&["ls", "cat", "curl"]));
    assert_eq!(command_names("2>&1 ls; >&2 echo x; &>/dev/null cat"), names(&["ls", "cat"]));
    assert_eq!(command_names("./node_modules/.bin/tsc --noEmit"), names(&["tsc"]));
}

#[test]
fn a_version_check_is_not_a_use_version_v_which_and_command_v_run_nothing_the_agent() {
    assert_eq!(command_names("gh --version && vercel -v; which java; command -v aws"), names(&[]));
    assert_eq!(command_names("java -version; python3 -V; pip -V && node -v"), names(&[]));
    assert_eq!(command_names("node --version | head -1; grep -v foo x.txt; pytest -v tests"), names(&["head", "grep", "pytest"]));
    assert_eq!(command_names("command -v gh >/dev/null 2>&1 || brew install gh"), names(&["brew"]));
    assert_eq!(install_names("command -v gh || npm i -g vercel"), installs(&[("npm", "vercel")]));
}

#[test]
fn keeps_an_unquoted_variable_inside_the_word_it_sits_in() {
    assert_eq!(command_names("echo ${HOME}/go/bin/x"), names(&[]));
    assert_eq!(command_names("echo ${HOME}/bin; ls ${PWD}"), names(&["ls"]));
    assert_eq!(command_names("${HOME}/go/bin/x --help && $BIN/y ${FLAGS:-${MORE}} z"), names(&["x", "y"]));
    assert_eq!(split_commands("a ${b|c} d"), names(&["a ${b|c} d"]));
}

#[test]
fn never_splits_inside_quotes_and_never_reads_a_heredoc_body() {
    assert_eq!(command_names("git commit -m 'fix: a | b && c'"), names(&["git"]));
    assert_eq!(command_names("sh -c \"ls | wc\""), names(&["sh"]));
    assert_eq!(command_names("python3 - <<'EOF'\nimport os; os.system('rm -rf /')\nEOF\ngit status"), names(&["python3", "git"]));
    assert_eq!(without_heredocs("cat <<EOF > x\nbrew install evil\nEOF"), "cat <<EOF > x");
    assert_eq!(split_commands("a 'b|c' d"), names(&["a b|c d"]));
}

#[test]
fn reads_the_packages_an_installer_was_asked_for_versions_off_one_shot_runners_unde() {
    assert_eq!(
        install_names("brew install gh jq && npm install -g agent-browser@0.31.1 wrangler"),
        installs(&[("brew", "gh"), ("brew", "jq"), ("npm", "agent-browser"), ("npm", "wrangler")])
    );
    assert_eq!(install_names("npm install typescript"), installs(&[]));
    assert_eq!(
        install_names("uv tool install aider-chat==0.86.2 && pipx install ruff && cargo install ripgrep"),
        installs(&[("uv", "aider-chat"), ("pipx", "ruff"), ("cargo", "ripgrep")])
    );
    assert_eq!(install_names("go install github.com/cli/cli/v2/cmd/gh@latest"), installs(&[("go", "gh")]));
    assert_eq!(
        install_names("npx -y @anthropic-ai/claude-code@1.0 --version; sudo apt-get install -y ffmpeg"),
        installs(&[("npx", "@anthropic-ai/claude-code"), ("apt", "ffmpeg")])
    );
}

#[test]
fn never_reads_a_redirection_target_as_a_package() {
    assert_eq!(
        install_names("brew install foo >/dev/null 2>&1 && npm i -g bar > out.txt 2> err.txt < in.txt"),
        installs(&[("brew", "foo"), ("npm", "bar")])
    );
    assert_eq!(
        install_names("brew install 2>&1 foo; brew install foo &>/dev/null; brew install foo >&2 bar"),
        installs(&[("brew", "foo"), ("brew", "foo"), ("brew", "foo"), ("brew", "bar")])
    );
}
