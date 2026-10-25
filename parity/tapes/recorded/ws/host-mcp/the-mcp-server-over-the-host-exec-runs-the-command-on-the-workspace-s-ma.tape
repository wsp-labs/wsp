{"meta":{"door":"ws","source":"packages/host/test/mcp.test.ts > the MCP server over the host > exec runs the command on the workspace's machine as argv and returns its output and exit code; a non-zero exit is a result, not an error"}}
{"in":{"token":"<redacted>","id":1,"op":"auth"}}
{"in":{"id":2,"op":"sessions.list"}}
{"in":{"workspaceId":"ws_7fa8a077","threadId":"b7b6ddc1-608b-44d5-8b3f-6a19bafeeb14","id":3,"op":"workspaces.get"}}
{"in":{"workspaceId":"ws_7fa8a077","id":4,"op":"workspaces.wake"}}
{"in":{"workspaceId":"ws_7fa8a077","argv":["sh","-c","printf 'one\\ntwo\\n'; exit 3"],"cwd":"/root/stub-7","id":5,"op":"workspaces.exec"}}
