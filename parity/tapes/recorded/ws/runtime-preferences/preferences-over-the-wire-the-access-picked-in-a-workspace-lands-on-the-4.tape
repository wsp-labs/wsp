{"meta":{"door":"ws","source":"packages/runtime/test/preferences.test.ts > preferences over the wire > the access picked in a workspace lands on the record and the next thread there reads it","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"patch":{"access":{"ws_b":3}},"id":2,"op":"preferences.set"}}
{"out":{"id":2,"ok":false,"error":"[\n  {\n    \"code\": \"invalid_type\",\n    \"expected\": \"string\",\n    \"received\": \"number\",\n    \"path\": [\n      \"patch\",\n      \"access\",\n      \"ws_b\"\n    ],\n    \"message\": \"Expected string, received number\"\n  }\n]"}}
