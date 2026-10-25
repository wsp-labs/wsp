{"meta":{"door":"ws","source":"packages/runtime/test/places-agents.test.ts > the agents on a computer you own > run a sign-in over that computer's link, push its steps to the asking socket alone, type its code, keep it going when that socket goes, and keep keys to the host's own socket","outs":"recorded"}}
{"in":{"token":"<redacted-1>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"target":{"placeId":"p_00000001"},"agent":"codex","id":2,"op":"agents.signInLine"}}
{"out":{"id":2,"ok":false,"error":"only the host's own command line asks for a sign-in's line; run wsp agents signin on the computer the host runs on"}}
{"in":{"agent":"claude","key":"<redacted-2>","id":3,"op":"agents.key"}}
{"out":{"id":3,"ok":false,"error":"only a socket holding this host's own token may put a key in its vault; use the app on the computer the host runs on, or wsp agents key there"}}
