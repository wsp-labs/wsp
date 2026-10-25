{"meta":{"door":"ws","source":"packages/host/test/verbs-exec-host.test.ts > wsp verbs over the host: wait, read, exec and the host itself > a wait in flight on a thread whose launch gives up gets that thread's finished line, with the reason the start failed with","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"init"}}
{"in":{"id":2,"op":"sessions.list"}}
{"out":{"id":2,"ok":true,"sessions":[{"id":"00000000-0000-4000-8000-000000000001","workspaceId":"ws_00000001","harness":"claude","status":"running","startedBy":"cli","threadId":"00000000-0000-4000-8000-000000000002","prompt":"build it","startedAt":1792928401479,"readAt":1792928401221,"project":{"id":"pr_00000002","name":"stub-1"},"computerName":"default"}]}}
{"in":{"id":3,"op":"events.subscribe"}}
{"out":{"id":3,"ok":true,"seq":7,"stream":"00000000-0000-4000-8000-000000000003"}}
{"in":{"id":4,"op":"sessions.list"}}
{"out":{"id":4,"ok":true,"sessions":[{"id":"00000000-0000-4000-8000-000000000001","workspaceId":"ws_00000001","harness":"claude","status":"running","startedBy":"cli","threadId":"00000000-0000-4000-8000-000000000002","prompt":"build it","startedAt":1792928401479,"readAt":1792928401221,"project":{"id":"pr_00000002","name":"stub-1"},"computerName":"default"}]}}
{"out":{"type":"session.end","workspaceId":"ws_00000001","sessionId":"00000000-0000-4000-8000-000000000001","turnId":"00000000-0000-4000-8000-000000000001","threadId":"00000000-0000-4000-8000-000000000002","exitCode":null,"sawResult":false,"reason":"the agent binary is not on this machine","at":1792928401599,"seq":8}}
{"out":{"type":"session.row","workspaceId":"ws_00000001","threadId":"00000000-0000-4000-8000-000000000002","id":"00000000-0000-4000-8000-000000000001","seq":8}}
{"in":{"workspaceId":"ws_00000001","threadId":"00000000-0000-4000-8000-000000000002","whole":true,"id":5,"op":"sessions.history"}}
{"out":{"id":5,"ok":true,"events":[]}}
