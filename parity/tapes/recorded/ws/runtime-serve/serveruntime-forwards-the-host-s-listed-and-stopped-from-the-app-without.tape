{"meta":{"door":"ws","source":"packages/runtime/test/serve.test.ts > serveRuntime forwards (the host's, listed and stopped from the app) > without a source the list is empty and a stop is refused","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"id":2,"op":"forwards.list"}}
{"out":{"id":2,"ok":true,"forwards":[]}}
{"in":{"workspaceId":"ws_1","port":8123,"id":3,"op":"forwards.stop"}}
{"out":{"id":3,"ok":false,"error":"nothing is forwarding localhost:8123 there"}}
