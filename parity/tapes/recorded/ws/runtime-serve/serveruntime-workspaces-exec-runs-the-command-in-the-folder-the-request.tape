{"meta":{"door":"ws","source":"packages/runtime/test/serve.test.ts > serveRuntime workspaces.exec > runs the command in the folder the request names, quoted for the machine's shell; without one, in the home, as a harness turn does","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"source":"https://github.com/wsp/wire-18.git","on":"default","id":2,"op":"projects.add"}}
{"out":{"id":2,"ok":true,"project":{"id":"pr_00000001","name":"wire-18","computer":"default","source":{"kind":"git","url":"https://github.com/wsp/wire-18.git"},"path":"/root/wire-18","remote":"https://github.com/wsp/wire-18.git","defaultBranch":"main","memoryKey":"-root-wire-18","memoryDir":"/root/.claude-cfg/projects/-root-wire-18/memory","createdAt":"2026-10-25T15:20:16.781Z"}}}
{"in":{"project":"pr_00000001","name":"x","golden":"snap_g","id":3,"op":"workspaces.create"}}
{"out":{"id":3,"ok":true,"workspace":{"id":"ws_00000002","name":"x","machineId":"m1","phase":"running","kind":"cloud","golden":"snap_g","createdAt":"2026-10-25T15:20:16.782Z","project":{"id":"pr_00000001","name":"wire-18","path":"/root/wire-18","computer":"default"},"home":"/root","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}}}
{"in":{"workspaceId":"ws_00000002","argv":["git","status"],"cwd":"/root/work/my proj","id":4,"op":"workspaces.exec"}}
{"out":{"id":4,"ok":true,"execId":"03b38bb7e8f7","cwd":"/root/work/my proj"}}
{"out":{"type":"exec.exit","execId":"03b38bb7e8f7","exitCode":0}}
{"in":{"workspaceId":"ws_00000002","argv":["git","status"],"id":5,"op":"workspaces.exec"}}
{"out":{"id":5,"ok":true,"execId":"fef57d5d89f8","cwd":"/root/wire-18"}}
{"out":{"type":"exec.exit","execId":"fef57d5d89f8","exitCode":0}}
