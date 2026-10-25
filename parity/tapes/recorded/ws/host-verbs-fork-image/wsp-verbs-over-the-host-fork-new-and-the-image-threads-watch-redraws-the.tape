{"meta":{"door":"ws","source":"packages/host/test/verbs-fork-image.test.ts > wsp verbs over the host: fork, new and the image > threads --watch redraws the same table where it stands until Ctrl-C, on one socket","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"init"}}
{"in":{"id":2,"op":"workspaces.list"}}
{"out":{"id":2,"ok":true,"workspaces":[{"id":"ws_00000001","name":"alpha","machineId":"m1","phase":"running","kind":"cloud","golden":"snap_gold","createdAt":"2026-10-25T11:41:04.202Z","project":{"id":"pr_00000002","name":"stub-2","path":"/root/stub-2","computer":"default"},"home":"/root","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}]}}
{"in":{"id":3,"op":"sessions.list"}}
{"out":{"id":3,"ok":true,"sessions":[]}}
{"in":{"id":4,"op":"workspaces.list"}}
{"out":{"id":4,"ok":true,"workspaces":[{"id":"ws_00000001","name":"alpha","machineId":"m1","phase":"running","kind":"cloud","golden":"snap_gold","createdAt":"2026-10-25T11:41:04.202Z","project":{"id":"pr_00000002","name":"stub-2","path":"/root/stub-2","computer":"default"},"home":"/root","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}]}}
{"in":{"id":5,"op":"sessions.list"}}
{"out":{"id":5,"ok":true,"sessions":[]}}
