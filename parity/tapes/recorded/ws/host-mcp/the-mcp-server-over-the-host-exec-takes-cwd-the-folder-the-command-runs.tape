{"meta":{"door":"ws","source":"packages/host/test/mcp.test.ts > the MCP server over the host > exec takes cwd, the folder the command runs in; without it the workspace's project folder; the result says which, and a relative one is refused"}}
{"in":{"token":"<redacted>","id":1,"op":"auth"}}
{"in":{"id":2,"op":"sessions.list"}}
{"in":{"workspaceId":"ws_00000001","threadId":"00000000-0000-4000-8000-000000000002","id":3,"op":"workspaces.get"}}
{"in":{"workspaceId":"ws_00000001","id":4,"op":"workspaces.wake"}}
{"in":{"workspaceId":"ws_00000001","argv":["git","status"],"cwd":"/root/stub-8","id":5,"op":"workspaces.exec"}}
{"in":{"id":6,"op":"sessions.list"}}
{"in":{"workspaceId":"ws_00000001","threadId":"00000000-0000-4000-8000-000000000002","id":7,"op":"workspaces.get"}}
{"in":{"workspaceId":"ws_00000001","id":8,"op":"workspaces.wake"}}
{"in":{"workspaceId":"ws_00000001","argv":["git","status"],"cwd":"/root/work/else where","id":9,"op":"workspaces.exec"}}
{"in":{"id":10,"op":"sessions.list"}}
{"in":{"workspaceId":"ws_00000001","threadId":"00000000-0000-4000-8000-000000000002","id":11,"op":"workspaces.get"}}
{"in":{"workspaceId":"ws_00000001","id":12,"op":"workspaces.wake"}}
{"in":{"workspaceId":"ws_00000001","argv":["git","status"],"cwd":"/root/stub-8","id":13,"op":"workspaces.exec"}}
