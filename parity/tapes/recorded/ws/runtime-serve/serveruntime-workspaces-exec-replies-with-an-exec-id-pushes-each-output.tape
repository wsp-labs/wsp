{"meta":{"door":"ws","source":"packages/runtime/test/serve.test.ts > serveRuntime workspaces.exec > replies with an exec id, pushes each output line to the asking socket and the exit code last, and pushes nothing to another socket","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"source":"https://github.com/wsp/wire-17.git","on":"default","id":2,"op":"projects.add"}}
{"out":{"id":2,"ok":true,"project":{"id":"pr_00000001","name":"wire-17","computer":"default","source":{"kind":"git","url":"https://github.com/wsp/wire-17.git"},"path":"/root/wire-17","remote":"https://github.com/wsp/wire-17.git","defaultBranch":"main","memoryKey":"-root-wire-17","memoryDir":"/root/.claude-cfg/projects/-root-wire-17/memory","createdAt":"2026-10-25T17:45:46.455Z"}}}
{"in":{"project":"pr_00000001","name":"x","golden":"snap_g","id":3,"op":"workspaces.create"}}
{"out":{"id":3,"ok":true,"workspace":{"id":"ws_00000002","name":"x","machineId":"m1","phase":"running","kind":"cloud","golden":"snap_g","createdAt":"2026-10-25T17:45:46.456Z","project":{"id":"pr_00000001","name":"wire-17","path":"/root/wire-17","computer":"default"},"home":"/root","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}}}
{"in":{"workspaceId":"ws_00000002","argv":["sh","-c","printf 'one\\ntwo\\n'; exit 3"],"id":4,"op":"workspaces.exec"}}
{"out":{"id":4,"ok":true,"execId":"289c80c2c755","cwd":"/root/wire-17"}}
{"out":{"type":"exec.output","execId":"289c80c2c755","text":"one"}}
{"out":{"type":"exec.output","execId":"289c80c2c755","text":"two"}}
{"out":{"type":"exec.exit","execId":"289c80c2c755","exitCode":3}}
{"in":{"workspaceId":"ws_nope","argv":["true"],"id":5,"op":"workspaces.exec"}}
{"out":{"id":5,"ok":false,"error":"no such workspace: ws_nope","kind":"not-found"}}
