{"meta":{"door":"ws","source":"packages/host/test/verbs-exec-host.test.ts > wsp verbs over the host: wait, read, exec and the host itself > exec runs the command where the thread works, streams its output and exits with its code","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"init"}}
{"in":{"id":2,"op":"sessions.list"}}
{"out":{"id":2,"ok":true,"sessions":[{"id":"00000000-0000-4000-8000-000000000001","workspaceId":"ws_00000001","harness":"claude","status":"completed","startedBy":"person","threadId":"00000000-0000-4000-8000-000000000002","prompt":"lead the work","startedAt":1792928401951,"model":"claude-opus-5-5","effort":"high","permissionMode":"bypassPermissions","cwd":"/root/stub-6","claudeSessionId":"00000000-0000-4000-8000-000000000001","lastLine":"re: lead the work","endedAt":1792928401957,"readAt":1792928401929,"project":{"id":"pr_00000002","name":"stub-6"},"computerName":"default"}]}}
{"in":{"workspaceId":"ws_00000001","threadId":"00000000-0000-4000-8000-000000000002","id":3,"op":"workspaces.get"}}
{"out":{"id":3,"ok":true,"workspace":{"id":"ws_00000001","name":"alpha","machineId":"m1","phase":"running","kind":"cloud","golden":"snap_gold","createdAt":"2026-10-25T11:40:01.942Z","project":{"id":"pr_00000002","name":"stub-6","path":"/root/stub-6","computer":"default"},"home":"/root","claudeSessionId":"00000000-0000-4000-8000-000000000001","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}}}
{"in":{"workspaceId":"ws_00000001","id":4,"op":"workspaces.wake"}}
{"out":{"id":4,"ok":true,"workspace":{"id":"ws_00000001","name":"alpha","machineId":"m1","phase":"running","kind":"cloud","golden":"snap_gold","createdAt":"2026-10-25T11:40:01.942Z","project":{"id":"pr_00000002","name":"stub-6","path":"/root/stub-6","computer":"default"},"home":"/root","claudeSessionId":"00000000-0000-4000-8000-000000000001","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}}}
{"in":{"workspaceId":"ws_00000001","argv":["sh","-c","printf 'one\\ntwo\\n'; exit 3"],"cwd":"/root/stub-6","id":5,"op":"workspaces.exec"}}
{"out":{"id":5,"ok":true,"execId":"734fe47d04ea","cwd":"/root/stub-6"}}
{"out":{"type":"exec.output","execId":"734fe47d04ea","text":"one"}}
{"out":{"type":"exec.output","execId":"734fe47d04ea","text":"two"}}
{"out":{"type":"exec.exit","execId":"734fe47d04ea","exitCode":3}}
