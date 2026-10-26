{"meta":{"door":"ws","source":"packages/runtime/test/spawn-guard.test.ts > agents spawning agents > a socket authed with a thread's token is stamped relayed and carries the scope, and may not pair a computer","outs":"recorded"}}
{"in":{"token":"<redacted-1>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"id":2,"op":"workspaces.list"}}
{"out":{"id":2,"ok":true,"workspaces":[{"id":"ws_00000001","name":"lead","machineId":"m1","phase":"running","kind":"cloud","golden":"snap_g","createdAt":"2026-10-25T17:45:26.567Z","project":{"id":"pr_00000002","name":"stub-35","path":"/root/stub-35","computer":"default"},"home":"/root","claudeSessionId":"00000000-0000-4000-8000-000000000001","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}]}}
{"in":{"workspaceId":"ws_00000003","name":"mini","id":3,"op":"workspaces.rename"}}
{"out":{"id":3,"ok":false,"error":"workspaces.rename is not a thread's to ask for; this request came in on the token of thread b2809155 on a machine wsp forks, and a thread's token starts work only under its own root and reads only that tree"}}
{"in":{"id":4,"op":"pair.issue"}}
{"out":{"id":4,"ok":false,"error":"pair.issue is not a thread's to ask for; this request came in on the token of thread b2809155 on a machine wsp forks, and a thread's token starts work only under its own root and reads only that tree"}}
{"in":{"source":"https://github.com/wsp/x.git","on":"default","id":5,"op":"projects.add"}}
{"out":{"id":5,"ok":false,"error":"projects.add is not a thread's to ask for; this request came in on the token of thread b2809155 on a machine wsp forks, and a thread's token starts work only under its own root and reads only that tree"}}
{"in":{"project":"pr_00000002","name":"builder","id":6,"op":"workspaces.create"}}
{"out":{"id":6,"ok":true,"workspace":{"id":"ws_00000004","name":"builder","machineId":"m2","phase":"running","kind":"cloud","golden":"snap_image-v1","createdAt":"2026-10-25T17:45:26.579Z","project":{"id":"pr_00000002","name":"stub-35","path":"/root/stub-35","computer":"default"},"home":"/root","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"parentThreadId":"00000000-0000-4000-8000-000000000002","rootThreadId":"00000000-0000-4000-8000-000000000002","parentWorkspaceId":"ws_00000001","provider":"default","pauseMode":"memory"},"notice":"builder starts on work as pushed"}}
