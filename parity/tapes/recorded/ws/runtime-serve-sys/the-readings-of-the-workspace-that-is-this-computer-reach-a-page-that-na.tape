{"meta":{"door":"ws","source":"packages/runtime/test/serve-sys.test.ts > the readings of the workspace that is this computer > reach a page that names this computer off the same sampler the workspace's panes ride","outs":"recorded","computer":"ip-172-31-43-224"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"placeId":"here","id":2,"op":"sys.subscribe"}}
{"out":{"id":2,"ok":true}}
{"in":{"placeId":"here","id":3,"op":"sys.subscribe"}}
{"out":{"id":3,"ok":true}}
{"in":{"workspaceId":"ws_00000001","id":4,"op":"sys.subscribe"}}
{"out":{"id":4,"ok":true}}
{"out":{"type":"place.sys","placeId":"here","sample":{"type":"sys.sample","cpu":12,"load1":0.4,"mem":{"used":1,"total":2},"disk":{"used":3,"total":4},"at":8}}}
{"out":{"type":"workspace.sys","workspaceId":"ws_00000001","sample":{"type":"sys.sample","cpu":12,"load1":0.4,"mem":{"used":1,"total":2},"disk":{"used":3,"total":4},"at":8}}}
{"in":{"placeId":"here","id":5,"op":"sys.unsubscribe"}}
{"out":{"id":5,"ok":true}}
{"out":{"type":"workspace.sys","workspaceId":"ws_00000001","sample":{"type":"sys.sample","cpu":12,"load1":0.4,"mem":{"used":1,"total":2},"disk":{"used":3,"total":4},"at":9}}}
{"in":{"placeId":"here","id":6,"op":"sys.subscribe"}}
{"out":{"id":6,"ok":true}}
