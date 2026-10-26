{"meta":{"door":"ws","source":"packages/host/test/place-door.test.ts > a request that arrives on the door > refuses a thread's token on the socket from 127.0.0.1, and the owner's own port still takes it","outs":"recorded"}}
{"in":{"token":"<redacted-1>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8"}}
{"in":{"id":2,"op":"workspaces.list"}}
{"out":{"id":2,"ok":true,"workspaces":[{"id":"ws_00000001","name":"lead","machineId":"m1","phase":"running","kind":"cloud","golden":"snap_gold","createdAt":"2026-10-25T17:49:05.240Z","project":{"id":"pr_00000002","name":"stub-1","path":"/root/stub-1","computer":"default"},"home":"/root","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}]}}
