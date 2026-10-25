{"meta":{"door":"ws","source":"packages/runtime/test/serve.test.ts > serveRuntime tickets (bearer never rides a URL after the handshake) > issues a 5-minute single-use connect ticket that authenticates a second socket","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"purpose":"connect","id":2,"op":"ticket.issue"}}
{"out":{"id":2,"ok":true,"ticket":"<redacted>","expiresAt":1300000}}
