{"meta":{"door":"ws","source":"packages/runtime/test/editor-door.test.ts > the editor ops over the wire > a server with no editor door refuses both ops in one line","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"workspaceId":"ws_copy","path":"/Users/dev/web/a.ts","id":2,"op":"editor.open"}}
{"out":{"id":2,"ok":false,"error":"this runtime cannot open an editor on this computer"}}
