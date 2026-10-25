{"meta":{"door":"ws","source":"packages/runtime/test/serve.test.ts > serveRuntime session history > answers a page of one thread with threadId, a thread's head with sessions.head, and refuses a window with no thread as usage"}}
{"in":{"token":"<redacted>","id":1,"op":"auth"}}
{"in":{"source":"https://github.com/wsp/wire-3.git","on":"default","id":2,"op":"projects.add"}}
{"in":{"project":"pr_59a5cde9","name":"x","golden":"snap_g","id":3,"op":"workspaces.create"}}
{"in":{"workspaceId":"ws_e47acac8","id":4,"op":"sessions.history"}}
{"in":{"workspaceId":"ws_e47acac8","threadId":"7dfc64e6-e385-4051-872f-d68b76a3589d","before":8,"limit":3,"id":5,"op":"sessions.history"}}
{"in":{"threadId":"7dfc64e6-e385-4051-872f-d68b76a3589d","id":6,"op":"sessions.head"}}
{"in":{"threadId":"no-such-thread","id":7,"op":"sessions.head"}}
{"in":{"workspaceId":"ws_e47acac8","limit":3,"id":8,"op":"sessions.history"}}
