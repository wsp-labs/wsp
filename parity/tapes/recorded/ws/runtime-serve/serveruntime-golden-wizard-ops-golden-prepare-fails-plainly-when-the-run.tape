{"meta":{"door":"ws","source":"packages/runtime/test/serve.test.ts > serveRuntime golden wizard ops > golden.prepare fails plainly when the runtime has no recipe","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"name":"default","id":2,"op":"golden.prepare"}}
{"out":{"id":2,"ok":false,"error":"this runtime has no golden recipe; the host wires one (setup + smoke) before the wizard can run"}}
