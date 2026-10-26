{"meta":{"door":"ws","source":"packages/runtime/test/pause.test.ts > the start guard > refuses sessions.start while pausing, paused and waking with the composer's sentence, in process and over the wire","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"workspaceId":"ws_00000001","prompt":"hi","id":2,"op":"sessions.start"}}
{"out":{"id":2,"ok":false,"error":"Workspace is paused; wake it to send"}}
