{"meta":{"door":"ws","source":"packages/runtime/test/serve-replay.test.ts > events.subscribe replay > runtime restarts between the drop and the re-subscribe: a cursor from the old stream is a gap, nothing of the new stream is replayed","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"source":"https://github.com/wsp/wire-7.git","on":"default","id":2,"op":"projects.add"}}
{"out":{"id":2,"ok":true,"project":{"id":"pr_00000001","name":"wire-7","computer":"default","source":{"kind":"git","url":"https://github.com/wsp/wire-7.git"},"path":"/root/wire-7","remote":"https://github.com/wsp/wire-7.git","defaultBranch":"main","memoryKey":"-root-wire-7","memoryDir":"/root/.claude-cfg/projects/-root-wire-7/memory","createdAt":"2026-10-25T17:49:26.482Z"}}}
{"in":{"project":"pr_00000001","name":"y","golden":"snap_g","id":3,"op":"workspaces.create"}}
{"out":{"id":3,"ok":true,"workspace":{"id":"ws_00000002","name":"y","machineId":"m1","phase":"running","kind":"cloud","golden":"snap_g","createdAt":"2026-10-25T17:49:26.512Z","project":{"id":"pr_00000001","name":"wire-7","path":"/root/wire-7","computer":"default"},"home":"/root","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}}}
{"in":{"workspaceId":"ws_00000002","prompt":"go","id":4,"op":"sessions.start"}}
{"out":{"id":4,"ok":true,"session":{"id":"00000000-0000-4000-8000-000000000001","workspaceId":"ws_00000002","harness":"claude","status":"running","startedBy":"person","threadId":"00000000-0000-4000-8000-000000000002","prompt":"go","startedAt":1792950566518,"model":"claude-sonnet-4-5","effort":"high","permissionMode":"bypassPermissions","cwd":"/root/wire-7","claudeSessionId":"00000000-0000-4000-8000-000000000001"},"outcome":"started","turnId":"00000000-0000-4000-8000-000000000003"}}
{"in":{"after":12,"stream":"00000000-0000-4000-8000-000000000004","id":5,"op":"events.subscribe"}}
{"out":{"id":5,"ok":true,"seq":19,"stream":"00000000-0000-4000-8000-000000000005","gap":true}}
{"out":{"type":"session.delta","workspaceId":"ws_00000002","sessionId":"00000000-0000-4000-8000-000000000001","turnId":"00000000-0000-4000-8000-000000000003","threadId":"00000000-0000-4000-8000-000000000002","line":4,"kind":"text","text":"live","at":1792950566526,"pos":5,"seq":20}}
