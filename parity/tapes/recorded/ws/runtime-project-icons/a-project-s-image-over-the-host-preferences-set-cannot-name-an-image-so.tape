{"meta":{"door":"ws","source":"packages/runtime/test/project-icons.test.ts > a project's image over the host > preferences.set cannot name an image, so no client names a hash the host does not hold","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"patch":{"projectIcon":{"pr_00000001":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}},"id":2,"op":"preferences.set"}}
{"out":{"id":2,"ok":false,"error":"[\n  {\n    \"code\": \"unrecognized_keys\",\n    \"keys\": [\n      \"projectIcon\"\n    ],\n    \"path\": [\n      \"patch\"\n    ],\n    \"message\": \"Unrecognized key(s) in object: 'projectIcon'\"\n  }\n]"}}
