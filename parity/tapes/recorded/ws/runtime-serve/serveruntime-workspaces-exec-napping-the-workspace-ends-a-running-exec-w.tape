{"meta":{"door":"ws","source":"packages/runtime/test/serve.test.ts > serveRuntime workspaces.exec > napping the workspace ends a running exec with the paused reason on the asking socket","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"source":"https://github.com/wsp/wire-22.git","on":"default","id":2,"op":"projects.add"}}
{"out":{"id":2,"ok":true,"project":{"id":"pr_00000001","name":"wire-22","computer":"default","source":{"kind":"git","url":"https://github.com/wsp/wire-22.git"},"path":"/root/wire-22","remote":"https://github.com/wsp/wire-22.git","defaultBranch":"main","memoryKey":"-root-wire-22","memoryDir":"/root/.claude-cfg/projects/-root-wire-22/memory","createdAt":"2026-10-25T11:38:12.217Z"}}}
{"in":{"project":"pr_00000001","name":"x","golden":"snap_g","id":3,"op":"workspaces.create"}}
{"out":{"id":3,"ok":true,"workspace":{"id":"ws_00000002","name":"x","machineId":"m1","phase":"running","kind":"cloud","golden":"snap_g","createdAt":"2026-10-25T11:38:12.218Z","project":{"id":"pr_00000001","name":"wire-22","path":"/root/wire-22","computer":"default"},"home":"/root","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}}}
{"in":{"workspaceId":"ws_00000002","argv":["sleep","600"],"id":4,"op":"workspaces.exec"}}
{"out":{"id":4,"ok":true,"execId":"69e043273c6c","cwd":"/root/wire-22"}}
{"in":{"workspaceId":"ws_00000002","id":5,"op":"workspaces.nap"}}
{"out":{"type":"exec.exit","execId":"69e043273c6c","exitCode":null,"error":"machine paused while the agent was working"}}
{"out":{"id":5,"ok":true,"workspace":{"id":"ws_00000002","name":"x","machineId":"m1","phase":"napping","kind":"cloud","golden":"snap_g","createdAt":"2026-10-25T11:38:12.218Z","project":{"id":"pr_00000001","name":"wire-22","path":"/root/wire-22","computer":"default"},"home":"/root","vaultedAt":"2026-10-25T11:38:12.231Z","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}}}
