{"meta":{"door":"ws","source":"packages/runtime/test/serve.test.ts > serveRuntime init door (the host's init job, read and driven from the app) > every init op reaches the door with its fields and answers with its view; a key never comes back"}}
{"in":{"token":"<redacted>","id":1,"op":"auth"}}
{"in":{"id":2,"op":"init.get"}}
{"in":{"provider":"box","key":"<redacted>","rows":{"logins/claude":"<redacted>"},"id":3,"op":"init.keys"}}
{"in":{"road":"agent","harness":"claude","id":4,"op":"init.start"}}
{"in":{"screen":"tools","ticks":["gh"],"answers":{"logins/gh":"machine"},"id":5,"op":"init.answer"}}
{"in":{"at":2,"id":6,"op":"init.step"}}
{"in":{"tool":"gh","id":7,"op":"init.retry"}}
{"in":{"firstWorkspace":"first","rebuild":true,"id":8,"op":"init.build"}}
{"in":{"tool":"gcloud","code":"<redacted>","id":9,"op":"init.signInCode"}}
{"in":{"id":10,"op":"init.cancel"}}
