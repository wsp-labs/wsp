{"meta":{"door":"ws","source":"packages/runtime/test/serve.test.ts > serveRuntime harness catalog > harnesses.list replies with one catalog per harness and sessions.start carries the picks through"}}
{"in":{"token":"<redacted>","id":1,"op":"auth"}}
{"in":{"id":2,"op":"harnesses.list"}}
{"in":{"source":"https://github.com/wsp/wire-5.git","on":"default","id":3,"op":"projects.add"}}
{"in":{"project":"pr_c61984d1","name":"x","golden":"snap_g","id":4,"op":"workspaces.create"}}
{"in":{"workspaceId":"ws_b2976e90","id":5,"op":"harnesses.list"}}
{"in":{"workspaceId":"ws_nope","id":6,"op":"harnesses.list"}}
{"in":{"workspaceId":"ws_b2976e90","prompt":"go","model":"claude-opus-5-5","effort":"high","permissionMode":"acceptEdits","contextWindow":"1m","fast":true,"requestId":"req_9","id":7,"op":"sessions.start"}}
{"in":{"workspaceId":"ws_b2976e90","id":8,"op":"sessions.history"}}
