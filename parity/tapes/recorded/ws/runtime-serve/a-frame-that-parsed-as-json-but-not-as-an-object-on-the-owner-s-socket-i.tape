{"meta":{"door":"ws","source":"packages/runtime/test/serve.test.ts > a frame that parsed as JSON but not as an object > on the owner's socket is refused the same way and the socket stays, answering the request after it","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":null}
{"out":{"id":null,"ok":false,"error":"a request is one JSON object, not a bare value or an array"}}
{"in":7}
{"out":{"id":null,"ok":false,"error":"a request is one JSON object, not a bare value or an array"}}
{"in":"a string"}
{"out":{"id":null,"ok":false,"error":"a request is one JSON object, not a bare value or an array"}}
{"in":true}
{"out":{"id":null,"ok":false,"error":"a request is one JSON object, not a bare value or an array"}}
{"in":[]}
{"out":{"id":null,"ok":false,"error":"a request is one JSON object, not a bare value or an array"}}
{"in":{"id":2,"op":"workspaces.list"}}
{"out":{"id":2,"ok":true,"workspaces":[]}}
