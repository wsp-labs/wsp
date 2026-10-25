{"meta":{"door":"ws","source":"packages/runtime/test/serve-replay.test.ts > events.subscribe replay > events emitted between a close and a re-subscribe arrive once and in order, then live continues","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"after":13,"id":2,"op":"events.subscribe"}}
{"out":{"id":2,"ok":true,"seq":16,"stream":"00000000-0000-4000-8000-000000000001"}}
{"out":{"type":"session.delta","workspaceId":"ws_00000001","sessionId":"00000000-0000-4000-8000-000000000002","turnId":"00000000-0000-4000-8000-000000000003","threadId":"00000000-0000-4000-8000-000000000004","line":3,"kind":"text","text":"dark 1","at":1792950565695,"pos":4,"seq":14}}
{"out":{"type":"session.delta","workspaceId":"ws_00000001","sessionId":"00000000-0000-4000-8000-000000000002","turnId":"00000000-0000-4000-8000-000000000003","threadId":"00000000-0000-4000-8000-000000000004","line":4,"kind":"text","text":"dark 2","at":1792950565695,"pos":5,"seq":15}}
{"out":{"type":"session.delta","workspaceId":"ws_00000001","sessionId":"00000000-0000-4000-8000-000000000002","turnId":"00000000-0000-4000-8000-000000000003","threadId":"00000000-0000-4000-8000-000000000004","line":5,"kind":"text","text":"dark 3","at":1792950565695,"pos":6,"seq":16}}
{"out":{"type":"session.delta","workspaceId":"ws_00000001","sessionId":"00000000-0000-4000-8000-000000000002","turnId":"00000000-0000-4000-8000-000000000003","threadId":"00000000-0000-4000-8000-000000000004","line":6,"kind":"text","text":"live","at":1792950565701,"pos":7,"seq":17}}
