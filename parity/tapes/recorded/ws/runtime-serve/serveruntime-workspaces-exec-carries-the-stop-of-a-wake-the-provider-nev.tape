{"meta":{"door":"ws","source":"packages/runtime/test/serve.test.ts > serveRuntime workspaces.exec > carries the stop of a wake the provider never took, so the row's own stop reaches the runtime","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"source":"https://github.com/wsp/wire-23.git","on":"default","id":2,"op":"projects.add"}}
{"out":{"id":2,"ok":true,"project":{"id":"pr_00000001","name":"wire-23","computer":"default","source":{"kind":"git","url":"https://github.com/wsp/wire-23.git"},"path":"/root/wire-23","remote":"https://github.com/wsp/wire-23.git","defaultBranch":"main","memoryKey":"-root-wire-23","memoryDir":"/root/.claude-cfg/projects/-root-wire-23/memory","createdAt":"2026-10-25T11:38:12.242Z"}}}
{"in":{"project":"pr_00000001","name":"x","golden":"snap_g","id":3,"op":"workspaces.create"}}
{"out":{"id":3,"ok":true,"workspace":{"id":"ws_00000002","name":"x","machineId":"m1","phase":"running","kind":"cloud","golden":"snap_g","createdAt":"2026-10-25T11:38:12.242Z","project":{"id":"pr_00000001","name":"wire-23","path":"/root/wire-23","computer":"default"},"home":"/root","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}}}
{"in":{"workspaceId":"ws_00000002","id":4,"op":"workspaces.nap"}}
{"out":{"id":4,"ok":true,"workspace":{"id":"ws_00000002","name":"x","machineId":"m1","phase":"napping","kind":"cloud","golden":"snap_g","createdAt":"2026-10-25T11:38:12.242Z","project":{"id":"pr_00000001","name":"wire-23","path":"/root/wire-23","computer":"default"},"home":"/root","vaultedAt":"2026-10-25T11:38:12.254Z","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}}}
{"in":{"workspaceId":"ws_00000002","id":5,"op":"workspaces.wake"}}
{"in":{"workspaceId":"ws_00000002","id":6,"op":"workspaces.stopWake"}}
{"out":{"id":5,"ok":false,"error":"waking stopped; the machine is still paused"}}
{"out":{"id":6,"ok":true,"workspace":{"id":"ws_00000002","name":"x","machineId":"m1","phase":"napping","kind":"cloud","golden":"snap_g","createdAt":"2026-10-25T11:38:12.242Z","project":{"id":"pr_00000001","name":"wire-23","path":"/root/wire-23","computer":"default"},"home":"/root","vaultedAt":"2026-10-25T11:38:12.254Z","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}}}
