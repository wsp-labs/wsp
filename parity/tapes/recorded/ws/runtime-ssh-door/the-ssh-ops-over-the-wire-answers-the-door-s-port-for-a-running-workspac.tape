{"meta":{"door":"ws","source":"packages/runtime/test/ssh-door.test.ts > the ssh ops over the wire > answers the door's port for a running workspace on another computer, and wakes a napping one first","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"workspaceId":"ws_fork","id":2,"op":"ssh.port"}}
{"out":{"id":2,"ok":true,"port":51022}}
