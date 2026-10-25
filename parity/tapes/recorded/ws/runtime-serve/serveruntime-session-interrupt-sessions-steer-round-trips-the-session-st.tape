{"meta":{"door":"ws","source":"packages/runtime/test/serve.test.ts > serveRuntime session interrupt > sessions.steer round-trips: the session.steer event reaches subscribers before the accepted reply, and history holds it"}}
{"in":{"token":"<redacted>","id":1,"op":"auth"}}
{"in":{"id":2,"op":"events.subscribe"}}
{"in":{"source":"https://github.com/wsp/wire-9.git","on":"default","id":3,"op":"projects.add"}}
{"in":{"project":"pr_1367ccfc","name":"x","golden":"snap_g","id":4,"op":"workspaces.create"}}
{"in":{"workspaceId":"ws_2598424c","prompt":"go","requestId":"req_1","id":5,"op":"sessions.start"}}
{"in":{"sessionId":"55555555-5555-4555-8555-555555555555","prompt":"and the tests","requestId":"req_2","id":6,"op":"sessions.steer"}}
{"in":{"workspaceId":"ws_2598424c","id":7,"op":"sessions.history"}}
{"in":{"sessionId":"55555555-5555-4555-8555-555555555555","prompt":"late","id":8,"op":"sessions.steer"}}
{"in":{"sessionId":"nope","prompt":"x","id":9,"op":"sessions.steer"}}
{"in":{"sessionId":"55555555-5555-4555-8555-555555555555","id":10,"op":"sessions.steer"}}
