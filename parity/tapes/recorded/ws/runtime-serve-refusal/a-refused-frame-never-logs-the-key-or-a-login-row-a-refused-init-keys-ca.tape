{"meta":{"door":"ws","source":"packages/runtime/test/serve-refusal.test.ts > a refused frame > never logs the key or a login row a refused init.keys carried, even when the refusal repeats it","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"provider":"box","key":"<redacted-1>","rows":{"logins/claude":"<redacted-2>"},"id":2,"op":"init.keys"}}
{"out":{"id":2,"ok":false,"error":"Box refused <redacted-1>\nits answer was 401. Check it and paste it again.","kind":"auth","fix":"Check it and paste it again."}}
