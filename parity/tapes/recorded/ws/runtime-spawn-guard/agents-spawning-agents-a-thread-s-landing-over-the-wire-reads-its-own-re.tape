{"meta":{"door":"ws","source":"packages/runtime/test/spawn-guard.test.ts > agents spawning agents > a thread's landing over the wire reads its own repository's projects and refuses another's by that rule","outs":"recorded"}}
{"in":{"token":"<redacted>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"project":"pr_00000001","id":2,"op":"workspaces.landing"}}
{"out":{"id":2,"ok":true,"name":"default","capabilities":{"liveCloneForks":true,"pauseMode":"memory","replacesMachine":true,"previewUrls":true,"signedUrls":true,"callbackRelay":true,"diskSnapshots":true,"images":true,"snapshotsAnyLife":false,"snapshotListing":true,"templates":false,"kept":false,"copies":true,"ownNetwork":true,"sizes":[{"cpu":2,"memMb":2048,"rateUsdPerHour":0.09},{"cpu":2,"memMb":4096,"rateUsdPerHour":0.11},{"cpu":2,"memMb":8192,"rateUsdPerHour":0.15},{"cpu":4,"memMb":8192,"rateUsdPerHour":0.22}]},"kind":"cloud"}}
{"in":{"project":"stub-19","id":3,"op":"workspaces.landing"}}
{"out":{"id":3,"ok":false,"error":"thread f4266714 works on stub-18, and stub-19 is a project of another repository; a thread starts children on its own repository's projects alone. Name a project of the same repository, or ask the person to start this one.","kind":"usage","fix":"Name a project of the same repository, or ask the person to start this one."}}
{"in":{"project":"nothing-here","id":4,"op":"workspaces.landing"}}
{"out":{"id":4,"ok":false,"error":"no project \"nothing-here\"","kind":"not-found"}}
