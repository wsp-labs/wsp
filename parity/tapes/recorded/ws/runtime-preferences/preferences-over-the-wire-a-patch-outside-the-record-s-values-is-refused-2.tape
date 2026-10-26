{"meta":{"door":"ws","source":"packages/runtime/test/preferences.test.ts > preferences over the wire > a patch outside the record's values is refused and the record stands","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"patch":{"sidebarWidth":0},"id":2,"op":"preferences.set"}}
{"out":{"id":2,"ok":false,"error":"[\n  {\n    \"code\": \"too_small\",\n    \"minimum\": 0,\n    \"type\": \"number\",\n    \"inclusive\": false,\n    \"exact\": false,\n    \"message\": \"Number must be greater than 0\",\n    \"path\": [\n      \"patch\",\n      \"sidebarWidth\"\n    ]\n  }\n]"}}
