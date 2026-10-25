{"meta":{"door":"ws","source":"packages/runtime/test/serve-refusal.test.ts > a refused frame > logs a frame the schema refused as its issues on one line, while the wire keeps its own sentence","outs":"recorded"}}
{"in":{"token":"<<host-token>>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"provider":42,"id":2,"op":"init.keys"}}
{"out":{"id":2,"ok":false,"error":"[\n  {\n    \"code\": \"invalid_type\",\n    \"expected\": \"string\",\n    \"received\": \"number\",\n    \"path\": [\n      \"provider\"\n    ],\n    \"message\": \"Expected string, received number\"\n  }\n]"}}
