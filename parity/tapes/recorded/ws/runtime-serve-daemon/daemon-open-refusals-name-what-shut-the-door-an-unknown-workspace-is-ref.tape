{"meta":{"door":"ws","source":"packages/runtime/test/serve-daemon.test.ts > daemon.open refusals name what shut the door > an unknown workspace is refused before anything is dialled","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"workspaceId":"ws_nobody","id":2,"op":"daemon.open"}}
{"out":{"id":2,"ok":false,"error":"no such workspace: ws_nobody","kind":"not-found"}}
