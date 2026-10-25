{"meta":{"door":"ws","source":"packages/runtime/test/runtime.test.ts > runtime > same behavior over the wire: serveRuntime round-trips create via WS","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"project":"pr_00000001","golden":"snap_g","name":"x","id":2,"op":"workspaces.create"}}
{"out":{"id":2,"ok":true,"workspace":{"id":"ws_00000002","name":"x","machineId":"m1","phase":"running","kind":"cloud","golden":"snap_g","createdAt":"2026-10-25T11:40:56.002Z","project":{"id":"pr_00000001","name":"stub-20","path":"/root/stub-20","computer":"default"},"home":"/root","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}}}
