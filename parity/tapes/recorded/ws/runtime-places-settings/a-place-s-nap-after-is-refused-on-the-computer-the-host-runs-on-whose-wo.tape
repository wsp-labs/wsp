{"meta":{"door":"ws","source":"packages/runtime/test/places-settings.test.ts > a place's nap after > is refused on the computer the host runs on, whose workspaces are folders that never nap, and says nothing of it on that row","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"placeId":"here","napMs":300000,"id":2,"op":"places.set"}}
{"out":{"id":2,"ok":false,"error":"zingzys-mac takes threads at once, turn limit, agents may start agents and levels deep, not nap after","kind":"usage"}}
{"in":{"placeId":"p_1","napMs":30000,"id":3,"op":"places.set"}}
{"out":{"id":3,"ok":false,"error":"[\n  {\n    \"code\": \"too_small\",\n    \"minimum\": 60000,\n    \"type\": \"number\",\n    \"inclusive\": true,\n    \"exact\": false,\n    \"message\": \"Number must be greater than or equal to 60000\",\n    \"path\": [\n      \"napMs\"\n    ]\n  }\n]"}}
