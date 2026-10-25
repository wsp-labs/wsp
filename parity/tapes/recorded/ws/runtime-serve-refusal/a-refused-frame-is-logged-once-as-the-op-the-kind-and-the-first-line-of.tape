{"meta":{"door":"ws","source":"packages/runtime/test/serve-refusal.test.ts > a refused frame > is logged once, as the op, the kind and the first line of the sentence","outs":"recorded"}}
{"in":{"token":"<<host-token>>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"workspaceId":"nope","id":2,"op":"workspaces.get"}}
{"out":{"id":2,"ok":false,"error":"no such workspace: nope","kind":"not-found"}}
