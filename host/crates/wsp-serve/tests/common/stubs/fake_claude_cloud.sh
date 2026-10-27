#!/bin/bash
sid=""
prev=""
for a in "$@"; do [ "$prev" = "--session-id" ] && sid="$a"; prev="$a"; done
here=$(dirname "$0")
mode=$(cat "$here/mode")
read -r first
say() { printf '%s\n' "$1"; }
say '{"type":"system","subtype":"init","cwd":"/root","session_id":"'"$sid"'","tools":["Bash"],"model":"claude-haiku-4-5","permissionMode":"bypassPermissions"}'
if [ "$mode" != "plain" ]; then
  say '{"type":"system","subtype":"background_tasks_changed","tasks":[{"task_id":"bk1","task_type":"local_bash","description":"pnpm exec vitest run"}],"session_id":"'"$sid"'"}'
  say '{"type":"system","subtype":"task_started","task_id":"bk1","tool_use_id":"toolu_1","description":"pnpm exec vitest run","is_backgrounded":true,"task_type":"local_bash","session_id":"'"$sid"'"}'
fi
say '{"type":"assistant","message":{"id":"msg_1","role":"assistant","model":"claude-haiku-4-5","content":[{"type":"text","text":"Pushed and reported."}],"usage":{"input_tokens":10,"output_tokens":5}},"parent_tool_use_id":null,"session_id":"'"$sid"'"}'
say '{"type":"result","subtype":"success","is_error":false,"duration_ms":1200,"num_turns":3,"result":"Pushed and reported.","total_cost_usd":0.01,"usage":{"input_tokens":10,"output_tokens":5},"session_id":"'"$sid"'"}'
# What the agent left behind, each holding the run's output (the log) open and outliving this process: one in the
# run's process group, as a test worker is, and one in a session of its own, as a server started with setsid is.
# Neither does any work.
sleep 600 &
setsid sleep 601 &
echo $! > "$here/escaped.pid"
# The CLI reads its input until EOF and exits; with a background task in its set it waits on that task first.
if [ "$mode" = "bg" ]; then wait; fi
if [ "$mode" = "steer" ] || [ "$mode" = "stop" ]; then
  read -r steered
  say '{"type":"system","subtype":"init","cwd":"/root","session_id":"'"$sid"'","tools":["Bash"],"model":"claude-haiku-4-5","permissionMode":"bypassPermissions"}'
  [ "$mode" = "steer" ] && exit 1
  say '{"type":"assistant","message":{"id":"msg_3","role":"assistant","model":"claude-haiku-4-5","content":[{"type":"text","text":"Looking at the flaky test."}],"usage":{"input_tokens":4,"output_tokens":5}},"parent_tool_use_id":null,"session_id":"'"$sid"'"}'
  sleep 600
fi
if [ "$mode" = "left" ]; then sleep 1; exit 0; fi
if [ "$mode" = "quiet" ] || [ "$mode" = "wake" ]; then
  sleep 4
  say '{"type":"system","subtype":"background_tasks_changed","tasks":[],"session_id":"'"$sid"'"}'
  say '{"type":"system","subtype":"task_notification","task_id":"bk1","tool_use_id":"toolu_1","status":"completed","summary":"Background command completed (exit code 0)","session_id":"'"$sid"'"}'
fi
if [ "$mode" = "wake" ]; then
  say '{"type":"system","subtype":"init","cwd":"/root","session_id":"'"$sid"'","tools":["Bash"],"model":"claude-haiku-4-5","permissionMode":"bypassPermissions"}'
  say '{"type":"assistant","message":{"id":"msg_2","role":"assistant","model":"claude-haiku-4-5","content":[{"type":"text","text":"The tests passed."}],"usage":{"input_tokens":4,"output_tokens":5}},"parent_tool_use_id":null,"session_id":"'"$sid"'"}'
  say '{"type":"result","subtype":"success","is_error":false,"duration_ms":900,"num_turns":1,"result":"The tests passed.","total_cost_usd":0.01,"usage":{"input_tokens":4,"output_tokens":5},"session_id":"'"$sid"'"}'
fi
cat > /dev/null
exit 0
