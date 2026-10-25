{"meta":{"door":"ws","source":"packages/runtime/test/slates.test.ts > a slate over the host's socket > answers a window's get, image, hold and release, and a refused state in its reply","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"threadId":"00000000-0000-4000-8000-000000000001","id":2,"op":"slates.get"}}
{"out":{"id":2,"ok":true,"slate":{"threadId":"00000000-0000-4000-8000-000000000001","workspaceId":"ws_00000001","version":1,"revision":1,"document":{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["image-1"]},"image-1":{"type":"image","props":{"src":"home.png"}}}},"values":{},"comments":[],"approvals":{},"asks":[],"problems":[],"shownOnce":false,"canUndo":false,"updatedAt":1792928326686}}}
{"in":{"threadId":"00000000-0000-4000-8000-000000000001","src":"home.png","id":3,"op":"slates.image"}}
{"out":{"id":3,"ok":true,"mediaType":"image/png","bytes":"iVBORw0KGgoAAAANSUhEUgAAAAEAAAAB","version":"1792928326681:24:2923914:1792928326681898241"}}
{"in":{"threadId":"00000000-0000-4000-8000-000000000001","src":"https://img.acme.test/a.png","id":4,"op":"slates.image"}}
{"out":{"id":4,"ok":true,"ask":{"domain":"img.acme.test"}}}
{"in":{"threadId":"00000000-0000-4000-8000-000000000001","sources":["git"],"id":5,"op":"slates.subscribe"}}
{"out":{"id":5,"ok":true}}
{"in":{"threadId":"00000000-0000-4000-8000-000000000001","sources":["git"],"id":6,"op":"slates.unsubscribe"}}
{"out":{"id":6,"ok":true}}
{"in":{"threadId":"00000000-0000-4000-8000-000000000001","values":{"$nope":1},"id":7,"op":"slates.state"}}
{"out":{"id":7,"ok":false,"error":"slate refused: 1 error: S501 name-undeclared $nope is not declared","kind":"invalid","errors":[{"code":"S501","name":"name-undeclared","message":"$nope is not declared"}],"warnings":[]}}
