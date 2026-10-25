{"meta":{"door":"ws","source":"packages/runtime/test/serve.test.ts > serveRuntime port reach > workspaces.rebuild replies with the workspace on a fresh golden fork and the old machine is dead","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"source":"https://github.com/wsp/wire-15.git","on":"default","id":2,"op":"projects.add"}}
{"out":{"id":2,"ok":true,"project":{"id":"pr_00000001","name":"wire-15","computer":"default","source":{"kind":"git","url":"https://github.com/wsp/wire-15.git"},"path":"/root/wire-15","remote":"https://github.com/wsp/wire-15.git","defaultBranch":"main","memoryKey":"-root-wire-15","memoryDir":"/root/.claude-cfg/projects/-root-wire-15/memory","createdAt":"2026-10-25T11:38:11.839Z"}}}
{"in":{"project":"pr_00000001","name":"x","golden":"snap_g","id":3,"op":"workspaces.create"}}
{"out":{"id":3,"ok":true,"workspace":{"id":"ws_00000002","name":"x","machineId":"m1","phase":"running","kind":"cloud","golden":"snap_g","createdAt":"2026-10-25T11:38:11.839Z","project":{"id":"pr_00000001","name":"wire-15","path":"/root/wire-15","computer":"default"},"home":"/root","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}}}
{"in":{"workspaceId":"ws_00000002","id":4,"op":"workspaces.rebuild"}}
{"out":{"id":4,"ok":true,"workspace":{"id":"ws_00000002","name":"x","machineId":"m2","phase":"running","kind":"cloud","golden":"snap_g","createdAt":"2026-10-25T11:38:11.839Z","project":{"id":"pr_00000001","name":"wire-15","path":"/root/wire-15","computer":"default"},"home":"/root","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}}}
{"in":{"workspaceId":"ws_nobody","id":5,"op":"workspaces.rebuild"}}
{"out":{"id":5,"ok":false,"error":"no such workspace: ws_nobody","kind":"not-found"}}
