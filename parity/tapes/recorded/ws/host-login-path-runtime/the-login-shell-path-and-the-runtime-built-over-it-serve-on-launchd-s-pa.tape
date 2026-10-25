{"meta":{"door":"ws","source":"packages/host/test/login-path-runtime.test.ts > the login shell PATH and the runtime built over it > serve on launchd's PATH builds its runtime after the read, so a turn on this computer runs under the person's PATH"}}
{"in":{"token":"<redacted>","id":1,"op":"auth"}}
{"in":{"id":2,"op":"sessions.list"}}
{"in":{"workspaceId":"ws_00000001","threadId":"00000000-0000-4000-8000-000000000002","id":3,"op":"workspaces.get"}}
{"in":{"workspaceId":"ws_00000001","id":4,"op":"workspaces.wake"}}
{"in":{"workspaceId":"ws_00000001","argv":["sh","-c","printf %s \"$PATH\""],"cwd":"<run>/wsp-login-path-000001","id":5,"op":"workspaces.exec"}}
