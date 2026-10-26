{"meta":{"door":"ws","source":"packages/runtime/test/ssh-door.test.ts > the ssh ops over the wire > refuses both ops on a socket a ticket let in","outs":"recorded"}}
{"in":{"workspaceId":"ws_fork","id":1,"op":"ssh.port"}}
{"out":{"id":1,"ok":false,"error":"a socket let in on a ticket cannot reach a workspace's ssh; use the wsp command or the app on the computer the host runs on","kind":"ticket"}}
{"in":{"on":true,"id":2,"op":"ssh.include"}}
{"out":{"id":2,"ok":false,"error":"a socket let in on a ticket cannot reach a workspace's ssh; use the wsp command or the app on the computer the host runs on","kind":"ticket"}}
