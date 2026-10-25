{"meta":{"door":"ws","source":"packages/runtime/test/serve-refusal.test.ts > a refused frame > a deeply nested secret field is refused and logged like any other frame, and the socket keeps answering","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":"<not written: RangeError>"}
{"out":{"id":90,"ok":false,"error":"[\n  {\n    \"code\": \"invalid_type\",\n    \"expected\": \"string\",\n    \"received\": \"array\",\n    \"path\": [\n      \"key\"\n    ],\n    \"message\": \"Expected string, received array\"\n  }\n]"}}
{"in":{"id":2,"op":"workspaces.list"}}
{"out":{"id":2,"ok":true,"workspaces":[]}}
