{"meta":{"door":"ws","source":"packages/runtime/test/serve.test.ts > serveRuntime session history > answers a page of one thread with threadId, a thread's head with sessions.head, and refuses a window with no thread as usage"}}
{"in":{"token":"<redacted>","id":1,"op":"auth"}}
{"in":{"source":"https://github.com/wsp/wire-3.git","on":"default","id":2,"op":"projects.add"}}
{"in":{"project":"pr_00000001","name":"x","golden":"snap_g","id":3,"op":"workspaces.create"}}
{"in":{"workspaceId":"ws_00000002","id":4,"op":"sessions.history"}}
{"in":{"workspaceId":"ws_00000002","threadId":"00000000-0000-4000-8000-000000000002","before":8,"limit":3,"id":5,"op":"sessions.history"}}
{"in":{"threadId":"00000000-0000-4000-8000-000000000002","id":6,"op":"sessions.head"}}
{"in":{"threadId":"no-such-thread","id":7,"op":"sessions.head"}}
{"in":{"workspaceId":"ws_00000002","limit":3,"id":8,"op":"sessions.history"}}
