{"meta":{"door":"ws","source":"packages/runtime/test/serve-replay.test.ts > events.subscribe replay > an in-ring cursor replays exactly the events after it, whether or not that socket ever saw them","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"after":14,"id":2,"op":"events.subscribe"}}
{"out":{"id":2,"ok":true,"seq":17,"stream":"00000000-0000-4000-8000-000000000001"}}
{"out":{"type":"session.delta","workspaceId":"ws_00000001","sessionId":"00000000-0000-4000-8000-000000000002","turnId":"00000000-0000-4000-8000-000000000003","threadId":"00000000-0000-4000-8000-000000000004","line":4,"kind":"text","text":"4","at":1792941821968,"pos":5,"seq":15}}
{"out":{"type":"session.delta","workspaceId":"ws_00000001","sessionId":"00000000-0000-4000-8000-000000000002","turnId":"00000000-0000-4000-8000-000000000003","threadId":"00000000-0000-4000-8000-000000000004","line":5,"kind":"text","text":"5","at":1792941821968,"pos":6,"seq":16}}
{"out":{"type":"session.delta","workspaceId":"ws_00000001","sessionId":"00000000-0000-4000-8000-000000000002","turnId":"00000000-0000-4000-8000-000000000003","threadId":"00000000-0000-4000-8000-000000000004","line":6,"kind":"text","text":"6","at":1792941821968,"pos":7,"seq":17}}
{"out":{"type":"session.delta","workspaceId":"ws_00000001","sessionId":"00000000-0000-4000-8000-000000000002","turnId":"00000000-0000-4000-8000-000000000003","threadId":"00000000-0000-4000-8000-000000000004","line":7,"kind":"text","text":"live","at":1792941821972,"pos":8,"seq":18}}
{"out":{"type":"session.delta","workspaceId":"ws_00000001","sessionId":"00000000-0000-4000-8000-000000000002","turnId":"00000000-0000-4000-8000-000000000003","threadId":"00000000-0000-4000-8000-000000000004","line":8,"kind":"text","text":"only this","at":1792941821976,"pos":9,"seq":19}}
