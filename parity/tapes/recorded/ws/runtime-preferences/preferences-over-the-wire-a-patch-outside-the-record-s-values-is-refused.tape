{"meta":{"door":"ws","source":"packages/runtime/test/preferences.test.ts > preferences over the wire > a patch outside the record's values is refused and the record stands","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"patch":{"theme":"sepia"},"id":2,"op":"preferences.set"}}
{"out":{"id":2,"ok":false,"error":"[\n  {\n    \"received\": \"sepia\",\n    \"code\": \"invalid_enum_value\",\n    \"options\": [\n      \"system\",\n      \"light\",\n      \"dark\"\n    ],\n    \"path\": [\n      \"patch\",\n      \"theme\"\n    ],\n    \"message\": \"Invalid enum value. Expected 'system' | 'light' | 'dark', received 'sepia'\"\n  }\n]"}}
