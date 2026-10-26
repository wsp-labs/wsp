{"meta":{"door":"ws","source":"packages/runtime/test/spawn-guard.test.ts > agents spawning agents > a token whose record names a machine's road, or no road at all, reaches no workspace on this computer","outs":"recorded"}}
{"in":{"token":"<redacted-1>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"id":2,"op":"workspaces.list"}}
{"out":{"id":2,"ok":true,"workspaces":[]}}
{"in":{"workspaceId":"ws_00000001","id":3,"op":"workspaces.get"}}
{"out":{"id":3,"ok":false,"error":"no such workspace","kind":"not-found"}}
