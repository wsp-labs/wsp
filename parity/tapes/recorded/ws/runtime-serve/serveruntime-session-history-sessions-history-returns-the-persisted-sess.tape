{"meta":{"door":"ws","source":"packages/runtime/test/serve.test.ts > serveRuntime session history > sessions.history returns the persisted session events for one workspace","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"source":"https://github.com/wsp/wire-2.git","on":"default","id":2,"op":"projects.add"}}
{"out":{"id":2,"ok":true,"project":{"id":"pr_00000001","name":"wire-2","computer":"default","source":{"kind":"git","url":"https://github.com/wsp/wire-2.git"},"path":"/root/wire-2","remote":"https://github.com/wsp/wire-2.git","defaultBranch":"main","memoryKey":"-root-wire-2","memoryDir":"/root/.claude-cfg/projects/-root-wire-2/memory","createdAt":"2026-10-25T11:38:11.239Z"}}}
{"in":{"project":"pr_00000001","name":"x","golden":"snap_g","id":3,"op":"workspaces.create"}}
{"out":{"id":3,"ok":true,"workspace":{"id":"ws_00000002","name":"x","machineId":"m1","phase":"running","kind":"cloud","golden":"snap_g","createdAt":"2026-10-25T11:38:11.253Z","project":{"id":"pr_00000001","name":"wire-2","path":"/root/wire-2","computer":"default"},"home":"/root","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}}}
{"in":{"workspaceId":"ws_00000002","id":4,"op":"sessions.history"}}
{"out":{"id":4,"ok":true,"events":[]}}
{"in":{"workspaceId":"ws_nope","id":5,"op":"sessions.history"}}
{"out":{"id":5,"ok":false,"error":"no such workspace: ws_nope","kind":"not-found"}}
