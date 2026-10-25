{"meta":{"door":"ws","source":"packages/runtime/test/serve.test.ts > serveRuntime workspaces.exec > the asking socket closing ends the command","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"source":"https://github.com/wsp/wire-20.git","on":"default","id":2,"op":"projects.add"}}
{"out":{"id":2,"ok":true,"project":{"id":"pr_00000001","name":"wire-20","computer":"default","source":{"kind":"git","url":"https://github.com/wsp/wire-20.git"},"path":"/root/wire-20","remote":"https://github.com/wsp/wire-20.git","defaultBranch":"main","memoryKey":"-root-wire-20","memoryDir":"/root/.claude-cfg/projects/-root-wire-20/memory","createdAt":"2026-10-25T11:38:12.160Z"}}}
{"in":{"project":"pr_00000001","name":"x","golden":"snap_g","id":3,"op":"workspaces.create"}}
{"out":{"id":3,"ok":true,"workspace":{"id":"ws_00000002","name":"x","machineId":"m1","phase":"running","kind":"cloud","golden":"snap_g","createdAt":"2026-10-25T11:38:12.167Z","project":{"id":"pr_00000001","name":"wire-20","path":"/root/wire-20","computer":"default"},"home":"/root","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}}}
{"in":{"workspaceId":"ws_00000002","argv":["sleep","600"],"id":4,"op":"workspaces.exec"}}
{"out":{"id":4,"ok":true,"execId":"655d5e64abe4","cwd":"/root/wire-20"}}
