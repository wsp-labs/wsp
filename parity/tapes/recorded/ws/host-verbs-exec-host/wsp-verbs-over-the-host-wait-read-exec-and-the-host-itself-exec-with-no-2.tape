{"meta":{"door":"ws","source":"packages/host/test/verbs-exec-host.test.ts > wsp verbs over the host: wait, read, exec and the host itself > exec with no --cwd runs in the folder the thread works in, a folder inside its project","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"init"}}
{"in":{"id":2,"op":"sessions.list"}}
{"out":{"id":2,"ok":true,"sessions":[{"id":"00000000-0000-4000-8000-000000000001","workspaceId":"ws_00000001","harness":"claude","status":"completed","startedBy":"person","threadId":"00000000-0000-4000-8000-000000000002","prompt":"lead the work","startedAt":1792928401827,"model":"claude-opus-5-5","effort":"high","permissionMode":"bypassPermissions","cwd":"/root/stub-5/packages","claudeSessionId":"00000000-0000-4000-8000-000000000001","lastLine":"re: lead the work","endedAt":1792928401839,"readAt":1792928401801,"project":{"id":"pr_00000002","name":"stub-5"},"computerName":"default"}]}}
{"in":{"workspaceId":"ws_00000001","threadId":"00000000-0000-4000-8000-000000000002","id":3,"op":"workspaces.get"}}
{"out":{"id":3,"ok":true,"workspace":{"id":"ws_00000001","name":"alpha","machineId":"m1","phase":"running","kind":"cloud","golden":"snap_gold","createdAt":"2026-10-25T11:40:01.814Z","project":{"id":"pr_00000002","name":"stub-5","path":"/root/stub-5","computer":"default"},"home":"/root","claudeSessionId":"00000000-0000-4000-8000-000000000001","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}}}
{"in":{"workspaceId":"ws_00000001","id":4,"op":"workspaces.wake"}}
{"out":{"id":4,"ok":true,"workspace":{"id":"ws_00000001","name":"alpha","machineId":"m1","phase":"running","kind":"cloud","golden":"snap_gold","createdAt":"2026-10-25T11:40:01.814Z","project":{"id":"pr_00000002","name":"stub-5","path":"/root/stub-5","computer":"default"},"home":"/root","claudeSessionId":"00000000-0000-4000-8000-000000000001","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}}}
{"in":{"workspaceId":"ws_00000001","argv":["true"],"cwd":"/root/stub-5","id":5,"op":"workspaces.exec"}}
{"out":{"id":5,"ok":true,"execId":"f585d95e329c","cwd":"/root/stub-5"}}
{"out":{"type":"exec.output","execId":"f585d95e329c","text":"ok"}}
{"out":{"type":"exec.exit","execId":"f585d95e329c","exitCode":0}}
