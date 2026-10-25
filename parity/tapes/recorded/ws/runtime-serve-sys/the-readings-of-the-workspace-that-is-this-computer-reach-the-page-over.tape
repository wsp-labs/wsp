{"meta":{"door":"ws","source":"packages/runtime/test/serve-sys.test.ts > the readings of the workspace that is this computer > reach the page over its own socket, with no daemon wired to dial","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"workspaceId":"ws_00000001","id":2,"op":"sys.subscribe"}}
{"out":{"id":2,"ok":true}}
{"out":{"type":"workspace.sys","workspaceId":"ws_00000001","sample":{"type":"sys.sample","cpu":12,"load1":0.4,"mem":{"used":1,"total":2},"disk":{"used":3,"total":4},"at":1}}}
{"in":{"workspaceId":"ws_00000001","id":3,"op":"daemon.open"}}
{"out":{"id":3,"ok":false,"error":"this host wired no daemon for this computer, so nothing on it can be dialled"}}
