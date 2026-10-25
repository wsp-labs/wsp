{"meta":{"door":"ws","source":"packages/runtime/test/places-facts.test.ts > a channel to the daemon on a computer you own > rides the link that computer opened: the frame goes up it, the answer comes back under the ask, and what it pushes reaches the socket that asked","outs":"recorded"}}
{"in":{"token":"<<host-token>>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"placeId":"p_00000001","id":2,"op":"daemon.open"}}
{"out":{"id":2,"ok":true,"channel":"86c0441b56b2"}}
{"in":{"channel":"86c0441b56b2","frame":{"op":"pty.create","cols":80,"rows":24,"env":{"CODEX_HOME":"/wsp/logins/codex"}},"id":3,"op":"daemon.send"}}
{"out":{"type":"daemon.event","channel":"86c0441b56b2","event":{"type":"pty.data","ptyId":"pty_7","data":"Open https://auth.openai.com/device"}}}
{"out":{"id":3,"ok":true,"reply":{"id":3,"ok":true,"ptyId":"pty_7"}}}
{"in":{"placeId":"p_00000001","workspaceId":"w_1","id":4,"op":"daemon.open"}}
{"out":{"id":4,"ok":false,"error":"daemon.open opens a channel to one daemon: name workspaceId or placeId, not both"}}
{"in":{"id":5,"op":"daemon.open"}}
{"out":{"id":5,"ok":false,"error":"daemon.open opens a channel to one daemon: name workspaceId or placeId, not both"}}
{"out":{"type":"daemon.closed","channel":"86c0441b56b2","code":1005,"reason":""}}
{"in":{"placeId":"p_00000001","id":6,"op":"daemon.open"}}
{"out":{"id":6,"ok":false,"error":"old-macbook is not answering; it connects on its own when it is on"}}
