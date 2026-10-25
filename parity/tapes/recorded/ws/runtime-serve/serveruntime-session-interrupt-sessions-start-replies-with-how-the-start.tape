{"meta":{"door":"ws","source":"packages/runtime/test/serve.test.ts > serveRuntime session interrupt > sessions.start replies with how the start went: started on a fresh thread, steered when the thread's turn runs and the harness steers, with the running turn as the session"}}
{"in":{"token":"<redacted>","id":1,"op":"auth"}}
{"in":{"id":2,"op":"events.subscribe"}}
{"in":{"source":"https://github.com/wsp/wire-11.git","on":"default","id":3,"op":"projects.add"}}
{"in":{"project":"pr_00000001","name":"x","golden":"snap_g","id":4,"op":"workspaces.create"}}
{"in":{"workspaceId":"ws_00000002","prompt":"go","requestId":"req_1","id":5,"op":"sessions.start"}}
{"in":{"workspaceId":"ws_00000002","prompt":"and STEERED","thread":"00000000-0000-4000-8000-000000000002","requestId":"req_2","startedBy":"cli","id":6,"op":"sessions.start"}}
{"in":{"workspaceId":"ws_00000002","prompt":"and BY THREAD","thread":"00000000-0000-4000-8000-000000000002","requestId":"req_3","id":7,"op":"sessions.start"}}
{"in":{"workspaceId":"ws_00000002","prompt":"nowhere","thread":"thr_nope","id":8,"op":"sessions.start"}}
