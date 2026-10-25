{"meta":{"door":"ws","source":"packages/runtime/test/serve.test.ts > serveRuntime workspaces.exec > refuses like sessions.start when no adapter is registered for the harness whose environment it would run with","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"source":"https://github.com/wsp/wire-19.git","on":"default","id":2,"op":"projects.add"}}
{"out":{"id":2,"ok":true,"project":{"id":"pr_00000001","name":"wire-19","computer":"default","source":{"kind":"git","url":"https://github.com/wsp/wire-19.git"},"path":"/root/wire-19","remote":"https://github.com/wsp/wire-19.git","defaultBranch":"main","memoryKey":"-root-wire-19","memoryDir":"/root/.claude-cfg/projects/-root-wire-19/memory","createdAt":"2026-10-25T17:45:46.528Z"}}}
{"in":{"project":"pr_00000001","name":"x","golden":"snap_g","id":3,"op":"workspaces.create"}}
{"out":{"id":3,"ok":true,"workspace":{"id":"ws_00000002","name":"x","machineId":"m1","phase":"running","kind":"cloud","golden":"snap_g","createdAt":"2026-10-25T17:45:46.529Z","project":{"id":"pr_00000001","name":"wire-19","path":"/root/wire-19","computer":"default"},"home":"/root","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}}}
{"in":{"workspaceId":"ws_00000002","argv":["true"],"id":4,"op":"workspaces.exec"}}
{"out":{"id":4,"ok":false,"error":"no adapter registered for harness \"claude\"; agents on this host: none"}}
