{"meta":{"door":"ws","source":"packages/runtime/test/serve.test.ts > serveRuntime golden wizard ops > golden.seal on a builder hydrated by a later process and found paused is refused as notFirstLife and the builder dies","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"name":"default","id":2,"op":"golden.prepare"}}
{"out":{"id":2,"ok":true,"builder":{"id":"m2","name":"default","kind":"sandbox","createdAt":"2026-10-25T17:45:46.386Z","placeId":"default","placeName":"default","size":{"cpu":2,"memMb":4096},"rateUsdPerHour":0.11000000000000001,"sealable":true}}}
{"in":{"builderId":"m1","id":3,"op":"golden.seal"}}
{"out":{"id":3,"ok":false,"error":"snapshot wsp-ip172313582h39caae4f-default-v1 refused: machine m1 is not first-life (it was resumed); snapshots only come from fresh machines","kind":"notFirstLife"}}
