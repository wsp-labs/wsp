{"meta":{"door":"ws","source":"packages/host/test/mcp.test.ts > the MCP server over the host > the workspace going away under a running exec ends the tool with the reason as an error","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"init"}}
{"in":{"id":2,"op":"sessions.list"}}
{"out":{"id":2,"ok":true,"sessions":[{"id":"00000000-0000-4000-8000-000000000001","workspaceId":"ws_00000001","harness":"claude","status":"completed","startedBy":"person","threadId":"00000000-0000-4000-8000-000000000002","prompt":"lead the work","startedAt":1792928287957,"model":"claude-opus-5-5","effort":"high","permissionMode":"bypassPermissions","cwd":"/root/stub-9","claudeSessionId":"00000000-0000-4000-8000-000000000001","lastLine":"re: lead the work","endedAt":1792928287959,"readAt":1792928287924,"project":{"id":"pr_00000002","name":"stub-9"},"computerName":"default"}]}}
{"in":{"workspaceId":"ws_00000001","threadId":"00000000-0000-4000-8000-000000000002","id":3,"op":"workspaces.get"}}
{"out":{"id":3,"ok":true,"workspace":{"id":"ws_00000001","name":"alpha","machineId":"m1","phase":"running","kind":"cloud","golden":"snap_gold","createdAt":"2026-10-25T11:38:07.944Z","project":{"id":"pr_00000002","name":"stub-9","path":"/root/stub-9","computer":"default"},"home":"/root","claudeSessionId":"00000000-0000-4000-8000-000000000001","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}}}
{"in":{"workspaceId":"ws_00000001","id":4,"op":"workspaces.wake"}}
{"out":{"id":4,"ok":true,"workspace":{"id":"ws_00000001","name":"alpha","machineId":"m1","phase":"running","kind":"cloud","golden":"snap_gold","createdAt":"2026-10-25T11:38:07.944Z","project":{"id":"pr_00000002","name":"stub-9","path":"/root/stub-9","computer":"default"},"home":"/root","claudeSessionId":"00000000-0000-4000-8000-000000000001","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}}}
{"in":{"workspaceId":"ws_00000001","argv":["sleep","600"],"cwd":"/root/stub-9","id":5,"op":"workspaces.exec"}}
{"out":{"id":5,"ok":true,"execId":"cfe665b1053e","cwd":"/root/stub-9"}}
{"out":{"type":"exec.exit","execId":"cfe665b1053e","exitCode":null,"error":"machine deleted while the agent was working"}}
