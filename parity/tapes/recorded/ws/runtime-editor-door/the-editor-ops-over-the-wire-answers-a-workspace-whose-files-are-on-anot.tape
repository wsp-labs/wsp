{"meta":{"door":"ws","source":"packages/runtime/test/editor-door.test.ts > the editor ops over the wire > answers a workspace whose files are on another machine with the one sentence where the host carries no ssh, and opens nothing","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"workspaceId":"ws_fork","path":"/root/wsp-boat/README.md","id":2,"op":"editor.open"}}
{"out":{"id":2,"ok":false,"error":"These files are on Delete compatibility and duplicates, so they open here."}}
