{"meta":{"door":"ws","source":"packages/runtime/test/serve.test.ts > serveRuntime auth > rejects malformed ops on an authed socket without closing it","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"id":2,"op":"workspaces.create"}}
{"out":{"id":2,"ok":false,"error":"[\n  {\n    \"code\": \"invalid_type\",\n    \"expected\": \"string\",\n    \"received\": \"undefined\",\n    \"path\": [\n      \"project\"\n    ],\n    \"message\": \"Required\"\n  },\n  {\n    \"code\": \"invalid_type\",\n    \"expected\": \"string\",\n    \"received\": \"undefined\",\n    \"path\": [\n      \"name\"\n    ],\n    \"message\": \"Required\"\n  }\n]"}}
{"in":{"id":3,"op":"workspaces.list"}}
{"out":{"id":3,"ok":true,"workspaces":[]}}
