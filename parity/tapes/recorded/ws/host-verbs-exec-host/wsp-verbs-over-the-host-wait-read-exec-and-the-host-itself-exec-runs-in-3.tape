{"meta":{"door":"ws","source":"packages/host/test/verbs-exec-host.test.ts > wsp verbs over the host: wait, read, exec and the host itself > exec runs in --cwd when given, else in the folder the thread works in; a failing command says on stderr where it ran"}}
{"in":{"token":"<redacted>","id":1,"op":"auth"}}
{"in":{"id":2,"op":"sessions.list"}}
{"in":{"workspaceId":"ws_00000001","threadId":"00000000-0000-4000-8000-000000000002","id":3,"op":"workspaces.get"}}
{"in":{"workspaceId":"ws_00000001","id":4,"op":"workspaces.wake"}}
{"in":{"workspaceId":"ws_00000001","argv":["git","status"],"cwd":"/root","id":5,"op":"workspaces.exec"}}
