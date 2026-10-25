{"meta":{"door":"ws","source":"packages/runtime/test/spawn-guard.test.ts > agents spawning agents > a token whose record names a machine's road, or no road at all, reaches no workspace on this computer","outs":"recorded"}}
{"in":{"token":"<redacted>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"id":2,"op":"workspaces.list"}}
{"out":{"id":2,"ok":true,"workspaces":[{"id":"ws_00000001","name":"lead","machineId":"m1","phase":"running","kind":"cloud","golden":"snap_g","createdAt":"2026-10-25T15:19:56.016Z","project":{"id":"pr_00000002","name":"stub-46","path":"/root/stub-46","computer":"default"},"home":"/root","claudeSessionId":"00000000-0000-4000-8000-000000000001","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}]}}
{"in":{"workspaceId":"ws_00000003","id":3,"op":"workspaces.get"}}
{"out":{"id":3,"ok":false,"error":"no such workspace","kind":"not-found"}}
