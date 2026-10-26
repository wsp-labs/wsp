{"meta":{"door":"ws","source":"packages/runtime/test/local-workspace.test.ts > local workspace > a machine's socket is stamped relayed by the host, whatever origin its client sends","outs":"recorded"}}
{"in":{"workspaceId":"ws_00000001","origin":"here","id":1,"op":"workspaces.get"}}
{"out":{"id":1,"ok":false,"error":"mac is this computer; it answers only requests from this computer, never one relayed from a machine"}}
{"in":{"origin":"here","id":2,"op":"workspaces.list"}}
{"out":{"id":2,"ok":true,"workspaces":[]}}
{"in":{"purpose":"connect","id":3,"op":"ticket.issue"}}
{"out":{"id":3,"ok":false,"error":"a request relayed from a machine cannot mint a ticket into this host","kind":"ticket"}}
