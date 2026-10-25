{"meta":{"door":"ws","source":"packages/runtime/test/serve.test.ts > a window that subscribes after a create began > hears the last word of every create still being made and every one refused since the host started, once, and a plain subscribe hears neither","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"source":"https://github.com/wsp/wire-24.git","on":"default","id":2,"op":"projects.add"}}
{"out":{"id":2,"ok":true,"project":{"id":"pr_00000001","name":"wire-24","computer":"default","source":{"kind":"git","url":"https://github.com/wsp/wire-24.git"},"path":"/root/wire-24","remote":"https://github.com/wsp/wire-24.git","defaultBranch":"main","memoryKey":"-root-wire-24","memoryDir":"/root/.claude-cfg/projects/-root-wire-24/memory","createdAt":"2026-10-25T15:20:16.953Z"}}}
{"in":{"project":"pr_00000001","name":"refused","golden":"snap_g","id":3,"op":"workspaces.create"}}
{"out":{"id":3,"ok":false,"error":"Snapshot not found","kind":"missing"}}
{"in":{"source":"https://github.com/wsp/wire-25.git","on":"default","id":4,"op":"projects.add"}}
{"out":{"id":4,"ok":true,"project":{"id":"pr_00000002","name":"wire-25","computer":"default","source":{"kind":"git","url":"https://github.com/wsp/wire-25.git"},"path":"/root/wire-25","remote":"https://github.com/wsp/wire-25.git","defaultBranch":"main","memoryKey":"-root-wire-25","memoryDir":"/root/.claude-cfg/projects/-root-wire-25/memory","createdAt":"2026-10-25T15:20:16.955Z"}}}
{"in":{"project":"pr_00000002","name":"held","golden":"snap_g","id":5,"op":"workspaces.create"}}
{"out":{"id":5,"ok":true,"workspace":{"id":"ws_00000003","name":"held","machineId":"m1","phase":"running","kind":"cloud","golden":"snap_g","createdAt":"2026-10-25T15:20:16.955Z","project":{"id":"pr_00000002","name":"wire-25","path":"/root/wire-25","computer":"default"},"home":"/root","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}}}
