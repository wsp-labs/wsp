{"meta":{"door":"ws","source":"packages/host/test/verbs-exec-host.test.ts > wsp verbs over the host: wait, read, exec and the host itself > a failing exec says the folder the host ran it in, which on this computer is the project's folder","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"init"}}
{"in":{"id":2,"op":"sessions.list"}}
{"out":{"id":2,"ok":true,"sessions":[{"id":"00000000-0000-4000-8000-000000000001","workspaceId":"ws_00000001","harness":"claude","status":"completed","startedBy":"person","threadId":"00000000-0000-4000-8000-000000000002","prompt":"lead the work","startedAt":1792950437512,"model":"claude-opus-5-5","effort":"high","permissionMode":"bypassPermissions","cwd":"<run>/wsp-exec-000001","claudeSessionId":"00000000-0000-4000-8000-000000000001","lastLine":"re: lead the work","endedAt":1792950437518,"readAt":1792950437474,"project":{"id":"pr_00000002","name":"mac"},"computerName":"ip-172-31-35-82"}]}}
{"in":{"workspaceId":"ws_00000001","threadId":"00000000-0000-4000-8000-000000000002","id":3,"op":"workspaces.get"}}
{"out":{"id":3,"ok":true,"workspace":{"id":"ws_00000001","name":"mac","machineId":"local","phase":"running","kind":"local","golden":"","createdAt":"2026-10-25T17:47:17.501Z","project":{"id":"pr_00000002","name":"mac","path":"<run>/wsp-exec-000001","computer":"here"},"folder":"<run>/wsp-exec-000001","home":"<run>/wsp-verbs-000002/user","claudeSessionId":"00000000-0000-4000-8000-000000000001","agents":{"spawn":true,"maxMachines":3,"maxDepth":2}}}}
{"in":{"workspaceId":"ws_00000001","id":4,"op":"workspaces.wake"}}
{"out":{"id":4,"ok":true,"workspace":{"id":"ws_00000001","name":"mac","machineId":"local","phase":"running","kind":"local","golden":"","createdAt":"2026-10-25T17:47:17.501Z","project":{"id":"pr_00000002","name":"mac","path":"<run>/wsp-exec-000001","computer":"here"},"folder":"<run>/wsp-exec-000001","home":"<run>/wsp-verbs-000002/user","claudeSessionId":"00000000-0000-4000-8000-000000000001","agents":{"spawn":true,"maxMachines":3,"maxDepth":2}}}}
{"in":{"workspaceId":"ws_00000001","argv":["false"],"cwd":"<run>/wsp-exec-000001","id":5,"op":"workspaces.exec"}}
{"out":{"id":5,"ok":true,"execId":"fe77c8f277ab","cwd":"<run>/wsp-exec-000001"}}
{"out":{"type":"exec.exit","execId":"fe77c8f277ab","exitCode":1}}
