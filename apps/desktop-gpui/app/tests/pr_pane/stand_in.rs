// SPDX-License-Identifier: AGPL-3.0-only
//! The claude the pane tests' hosts run: it answers each prompt at once with one line, in the folder it was started in,
//! or in the folder `$HOME/next-cwd` names, so a test can move a thread's next turn.

pub const STAND_IN: &str = r#"#!/bin/sh
case " $* " in
  *" --version "*) echo "2.1.295 (Claude Code)"; exit 0 ;;
  *" --help "*) echo '--permission-mode <mode>  (choices: "default", "bypassPermissions")'; exit 0 ;;
  *" --bare "*) read -r _; echo '{"type":"control_response","response":{"subtype":"success","request_id":"init","response":{"models":[{"value":"default","resolvedModel":"claude-haiku-5-5"}]}}}'; exit 0 ;;
  *" --input-format stream-json "*) ;;
  *) exit 1 ;;
esac
session=""; prev=""
for a in "$@"; do case $prev in --session-id|--resume) session=$a ;; esac; prev=$a; done
while IFS= read -r _; do
  cwd=$(pwd); [ -f "$HOME/next-cwd" ] && cwd=$(cat "$HOME/next-cwd")
  printf '{"type":"system","subtype":"init","session_id":"%s","model":"claude-haiku-5-5","cwd":"%s","tools":[]}\n' "$session" "$cwd"
  printf '{"type":"assistant","session_id":"%s","message":{"role":"assistant","content":[{"type":"text","text":"ok"}]}}\n' "$session"
  printf '{"type":"result","subtype":"success","is_error":false,"result":"ok","session_id":"%s"}\n' "$session"
done
"#;
