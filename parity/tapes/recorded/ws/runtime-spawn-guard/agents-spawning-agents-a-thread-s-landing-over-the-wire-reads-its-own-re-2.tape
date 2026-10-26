{"meta":{"door":"ws","source":"packages/runtime/test/spawn-guard.test.ts > agents spawning agents > a thread's landing over the wire reads its own repository's projects and refuses another's by that rule","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"project":"nothing-here","id":2,"op":"workspaces.landing"}}
{"out":{"id":2,"ok":false,"error":"no project \"nothing-here\"; this host holds stub-18, stub-19","kind":"not-found"}}
