{"meta":{"door":"ws","source":"packages/runtime/test/ssh-door.test.ts > the ssh ops over the wire > a server with no ssh door refuses both ops in one line","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"on":true,"id":2,"op":"ssh.include"}}
{"out":{"id":2,"ok":false,"error":"this runtime cannot carry an editor's ssh from this computer"}}
