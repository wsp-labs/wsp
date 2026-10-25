{"meta":{"door":"ws","source":"packages/runtime/test/serve-replay.test.ts > events.subscribe replay > events emitted between a close and a re-subscribe arrive once and in order, then live continues","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"source":"https://github.com/wsp/wire-2.git","on":"default","id":2,"op":"projects.add"}}
{"out":{"id":2,"ok":true,"project":{"id":"pr_00000001","name":"wire-2","computer":"default","source":{"kind":"git","url":"https://github.com/wsp/wire-2.git"},"path":"/root/wire-2","remote":"https://github.com/wsp/wire-2.git","defaultBranch":"main","memoryKey":"-root-wire-2","memoryDir":"/root/.claude-cfg/projects/-root-wire-2/memory","createdAt":"2026-10-25T15:23:41.931Z"}}}
{"in":{"project":"pr_00000001","name":"x","golden":"snap_g","id":3,"op":"workspaces.create"}}
{"out":{"id":3,"ok":true,"workspace":{"id":"ws_00000002","name":"x","machineId":"m1","phase":"running","kind":"cloud","golden":"snap_g","createdAt":"2026-10-25T15:23:41.938Z","project":{"id":"pr_00000001","name":"wire-2","path":"/root/wire-2","computer":"default"},"home":"/root","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}}}
{"in":{"workspaceId":"ws_00000002","prompt":"go","id":4,"op":"sessions.start"}}
{"out":{"id":4,"ok":true,"session":{"id":"00000000-0000-4000-8000-000000000001","workspaceId":"ws_00000002","harness":"claude","status":"running","startedBy":"person","threadId":"00000000-0000-4000-8000-000000000002","prompt":"go","startedAt":1792941821945,"model":"claude-sonnet-4-5","effort":"high","permissionMode":"bypassPermissions","cwd":"/root/wire-2","claudeSessionId":"00000000-0000-4000-8000-000000000001"},"outcome":"started","turnId":"00000000-0000-4000-8000-000000000003"}}
{"in":{"id":5,"op":"events.subscribe"}}
{"out":{"id":5,"ok":true,"seq":11,"stream":"00000000-0000-4000-8000-000000000004"}}
{"out":{"type":"session.delta","workspaceId":"ws_00000002","sessionId":"00000000-0000-4000-8000-000000000001","turnId":"00000000-0000-4000-8000-000000000003","threadId":"00000000-0000-4000-8000-000000000002","line":1,"kind":"text","text":"a","at":1792941821947,"pos":2,"seq":12}}
{"out":{"type":"session.delta","workspaceId":"ws_00000002","sessionId":"00000000-0000-4000-8000-000000000001","turnId":"00000000-0000-4000-8000-000000000003","threadId":"00000000-0000-4000-8000-000000000002","line":2,"kind":"text","text":"b","at":1792941821947,"pos":3,"seq":13}}
