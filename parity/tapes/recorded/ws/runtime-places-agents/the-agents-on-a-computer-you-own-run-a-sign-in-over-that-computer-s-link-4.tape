{"meta":{"door":"ws","source":"packages/runtime/test/places-agents.test.ts > the agents on a computer you own > run a sign-in over that computer's link, push its steps to the asking socket alone, type its code, keep it going when that socket goes, and keep keys to the host's own socket","outs":"recorded"}}
{"in":{"target":{"placeId":"p_00000001"},"agent":"codex","id":1,"op":"agents.signIn"}}
{"out":{"id":1,"ok":false,"error":"a socket let in on a ticket cannot see or change the places this host holds; run wsp computers on the computer the host runs on","kind":"ticket"}}
{"in":{"signInId":"si_00000002","code":"<redacted-1>","id":2,"op":"agents.signInCode"}}
{"out":{"id":2,"ok":false,"error":"a socket let in on a ticket cannot see or change the places this host holds; run wsp computers on the computer the host runs on","kind":"ticket"}}
{"in":{"signInId":"si_00000002","id":3,"op":"agents.signInStop"}}
{"out":{"id":3,"ok":false,"error":"a socket let in on a ticket cannot see or change the places this host holds; run wsp computers on the computer the host runs on","kind":"ticket"}}
{"in":{"target":{"placeId":"p_00000001"},"agent":"codex","id":4,"op":"agents.signInLine"}}
{"out":{"id":4,"ok":false,"error":"a socket let in on a ticket cannot see or change the places this host holds; run wsp computers on the computer the host runs on","kind":"ticket"}}
{"in":{"target":{"placeId":"here"},"agent":"codex","id":5,"op":"agents.addTools"}}
{"out":{"id":5,"ok":false,"error":"a socket let in on a ticket cannot see or change the places this host holds; run wsp computers on the computer the host runs on","kind":"ticket"}}
{"in":{"agent":"claude","key":"<redacted-2>","id":6,"op":"agents.key"}}
{"out":{"id":6,"ok":false,"error":"only a socket holding this host's own token may put a key in its vault; use the app on the computer the host runs on, or wsp agents key there"}}
{"in":{"id":7,"op":"agents.signIns"}}
{"out":{"id":7,"ok":false,"error":"a socket let in on a ticket cannot see or change the places this host holds; run wsp computers on the computer the host runs on","kind":"ticket"}}
