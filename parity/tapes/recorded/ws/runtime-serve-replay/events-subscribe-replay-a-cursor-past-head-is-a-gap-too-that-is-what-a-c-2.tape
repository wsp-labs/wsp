{"meta":{"door":"ws","source":"packages/runtime/test/serve-replay.test.ts > events.subscribe replay > a cursor past head is a gap too: that is what a cursor from a runtime that restarted looks like","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"after":90,"id":2,"op":"events.subscribe"}}
{"out":{"id":2,"ok":true,"seq":11,"stream":"00000000-0000-4000-8000-000000000001","gap":true}}
{"out":{"type":"session.delta","workspaceId":"ws_00000001","sessionId":"00000000-0000-4000-8000-000000000002","turnId":"00000000-0000-4000-8000-000000000003","threadId":"00000000-0000-4000-8000-000000000004","line":1,"kind":"text","text":"live","at":1792950566479,"pos":2,"seq":12}}
