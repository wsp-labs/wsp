{"meta":{"door":"ws","source":"packages/runtime/test/spawn-guard.test.ts > agents spawning agents > a thread on this computer starts its threads beside it, held to the computer's default depth and no machine cap, and the person's doors stay shut","outs":"recorded"}}
{"in":{"token":"<redacted-1>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"prompt":"deepest","id":2,"op":"sessions.start"}}
{"out":{"id":2,"ok":false,"error":"thread 043f5739 is 2 deep under its root and this computer allows 2, so a thread this deep may not spawn; raise Levels deep in Settings > Computers or run wsp computers set this-mac --max-depth 3"}}
