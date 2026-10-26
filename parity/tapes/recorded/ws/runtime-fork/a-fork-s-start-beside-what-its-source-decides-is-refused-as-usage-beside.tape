{"meta":{"door":"ws","source":"packages/runtime/test/fork.test.ts > a fork's start beside what its source decides > is refused as usage beside a conversation kept outside wsp, in process and over the wire, before anything starts","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"prompt":"x","fork":{"threadId":"00000000-0000-4000-8000-000000000001"},"resume":{"id":"00000000-0000-4000-8000-000000000002","copy":true},"id":2,"op":"sessions.start"}}
{"out":{"id":2,"ok":false,"error":"a fork carries a thread's conversation and --resume carries one kept outside wsp, so a start takes one of them. Name the thread with --fork, or the conversation with --resume and --copy, not both.","kind":"usage","fix":"Name the thread with --fork, or the conversation with --resume and --copy, not both."}}
