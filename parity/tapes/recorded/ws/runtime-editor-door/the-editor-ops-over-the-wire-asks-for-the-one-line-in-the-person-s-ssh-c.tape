{"meta":{"door":"ws","source":"packages/runtime/test/editor-door.test.ts > the editor ops over the wire > asks for the one line in the person's ssh config before any ssh, and opens nothing until it stands","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"workspaceId":"ws_fork","path":"/root/wsp-boat/src/cart.ts","id":2,"op":"editor.open"}}
{"out":{"id":2,"ok":false,"error":"Delete compatibility and duplicates opens in your editor over ssh, which needs one line at the top of ~/.ssh/config.","kind":"sshInclude"}}
