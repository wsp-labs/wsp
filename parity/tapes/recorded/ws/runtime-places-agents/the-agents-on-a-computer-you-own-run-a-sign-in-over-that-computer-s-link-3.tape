{"meta":{"door":"ws","source":"packages/runtime/test/places-agents.test.ts > the agents on a computer you own > run a sign-in over that computer's link, push its steps to the asking socket alone, type its code, keep it going when that socket goes, and keep keys to the host's own socket","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"id":2,"op":"events.subscribe"}}
{"out":{"id":2,"ok":true,"seq":2,"stream":"00000000-0000-4000-8000-000000000001"}}
{"out":{"type":"agents.changed","seq":3}}
{"out":{"type":"agents.changed","seq":4}}
{"in":{"signInId":"si_00000001","code":"<redacted-1>","id":3,"op":"agents.signInCode"}}
{"out":{"id":3,"ok":false,"error":"That sign-in is not running any more.","kind":"usage"}}
{"in":{"signInId":"si_00000001","id":4,"op":"agents.signInStop"}}
{"out":{"id":4,"ok":false,"error":"That sign-in is not running any more.","kind":"usage"}}
{"out":{"type":"agents.changed","target":{"placeId":"p_00000002"},"seq":5}}
{"in":{"target":{"placeId":"p_00000002"},"agent":"codex","id":5,"op":"agents.signIn"}}
{"out":{"id":5,"ok":true,"signInId":"si_00000003"}}
{"out":{"type":"agents.signIn","signInId":"si_00000003","state":"waiting","url":"https://auth.openai.com/codex/device","code":"<redacted-2>","paste":false}}
{"in":{"signInId":"si_00000003","id":6,"op":"agents.signInStop"}}
{"out":{"id":6,"ok":true}}
{"out":{"type":"agents.changed","target":{"placeId":"p_00000002"},"seq":6}}
{"in":{"id":7,"op":"agents.signIns"}}
{"out":{"id":7,"ok":true,"runs":[{"signInId":"si_00000004","target":{"placeId":"p_00000002"},"agent":"codex","server":"notion","scope":"project","project":"pr_1","last":{"type":"agents.signIn","signInId":"si_00000004","state":"waiting","url":"https://auth.openai.com/codex/device","code":"<redacted-2>","paste":false}},{"signInId":"si_00000001","target":{"placeId":"p_00000002"},"agent":"codex","last":{"type":"agents.signIn","signInId":"si_00000001","state":"waiting","url":"https://auth.openai.com/codex/device","code":"<redacted-2>","paste":false},"ended":true},{"signInId":"si_00000003","target":{"placeId":"p_00000002"},"agent":"codex","last":{"type":"agents.signIn","signInId":"si_00000003","state":"waiting","url":"https://auth.openai.com/codex/device","code":"<redacted-2>","paste":false},"ended":true}]}}
{"out":{"type":"agents.signIn","signInId":"si_00000004","state":"waiting","url":"https://auth.openai.com/codex/device","code":"<redacted-2>","paste":false}}
{"out":{"type":"agents.changed","target":{"placeId":"p_00000002"},"seq":7}}
