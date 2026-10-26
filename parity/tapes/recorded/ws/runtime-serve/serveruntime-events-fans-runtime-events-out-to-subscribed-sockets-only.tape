{"meta":{"door":"ws","source":"packages/runtime/test/serve.test.ts > serveRuntime events > fans runtime events out to subscribed sockets only","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"id":2,"op":"events.subscribe"}}
{"out":{"id":2,"ok":true,"seq":0,"stream":"00000000-0000-4000-8000-000000000001"}}
{"in":{"source":"https://github.com/wsp/wire-1.git","on":"default","id":3,"op":"projects.add"}}
{"out":{"type":"project.added","project":{"id":"pr_00000001","name":"wire-1","computer":"default","source":{"kind":"git","url":"https://github.com/wsp/wire-1.git"},"path":"/root/wire-1","remote":"https://github.com/wsp/wire-1.git","defaultBranch":"main","memoryKey":"-root-wire-1","memoryDir":"/root/.claude-cfg/projects/-root-wire-1/memory","createdAt":"2026-10-25T17:45:46.042Z"},"seq":1}}
{"out":{"id":3,"ok":true,"project":{"id":"pr_00000001","name":"wire-1","computer":"default","source":{"kind":"git","url":"https://github.com/wsp/wire-1.git"},"path":"/root/wire-1","remote":"https://github.com/wsp/wire-1.git","defaultBranch":"main","memoryKey":"-root-wire-1","memoryDir":"/root/.claude-cfg/projects/-root-wire-1/memory","createdAt":"2026-10-25T17:45:46.042Z"}}}
{"in":{"project":"pr_00000001","name":"x","golden":"snap_g","id":4,"op":"workspaces.create"}}
{"out":{"type":"workspace.creating","workspaceId":"ws_00000002","name":"x","stage":"fork-requested","message":"starting x on default","elapsedMs":1,"seq":2}}
{"out":{"type":"workspace.creating","workspaceId":"ws_00000002","name":"x","stage":"hostname-set","message":"hostname set to x","elapsedMs":3,"seq":3}}
{"out":{"type":"workspace.creating","workspaceId":"ws_00000002","name":"x","stage":"project-cloned","message":"Cloning https://github.com/wsp/wire-1.git into /root/wire-1.","elapsedMs":25,"seq":4}}
{"out":{"type":"workspace.creating","workspaceId":"ws_00000002","name":"x","stage":"ready","message":"ready","elapsedMs":26,"seq":5}}
{"out":{"type":"workspace.created","workspace":{"home":"/root","id":"ws_00000002","name":"x","machineId":"m1","phase":"running","kind":"cloud","golden":"snap_g","createdAt":"2026-10-25T17:45:46.045Z","project":{"id":"pr_00000001","name":"wire-1","path":"/root/wire-1","computer":"default"},"agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"},"seq":6}}
{"out":{"id":4,"ok":true,"workspace":{"id":"ws_00000002","name":"x","machineId":"m1","phase":"running","kind":"cloud","golden":"snap_g","createdAt":"2026-10-25T17:45:46.045Z","project":{"id":"pr_00000001","name":"wire-1","path":"/root/wire-1","computer":"default"},"home":"/root","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}}}
{"out":{"type":"workspace.cost","workspaceId":"ws_00000002","phase":"running","rateUsdPerHour":0.11000000000000001,"awakeMs":0,"accruedUsd":0,"at":"2026-10-25T17:45:46.071Z","seq":7}}
