{"meta":{"door":"ws","source":"packages/runtime/test/preferences.test.ts > preferences over the wire > an op that answers with an error is told to the server's failed hook, by op and with the error as thrown","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"patch":{"defaultAgent":"nope"},"id":2,"op":"preferences.set"}}
{"out":{"id":2,"ok":false,"error":"no adapter registered for harness \"nope\"; agents on this host: none","kind":"usage"}}
