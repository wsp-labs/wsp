{"meta":{"door":"ws","source":"packages/runtime/test/serve.test.ts > serveRuntime snapshot lineage > snapshots.rollback to a version outside the manifest is a typed refusal that changes nothing","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"version":9,"id":2,"op":"snapshots.rollback"}}
{"out":{"id":2,"ok":false,"error":"rollback target v9 not in manifest","kind":"missing"}}
{"in":{"id":3,"op":"snapshots.list"}}
{"out":{"id":3,"ok":true,"lineage":{"name":"default","head":1,"versions":[{"version":1,"snapshotId":"snap_golden-v1","baseTemplate":"base","setupSha":"sha1","createdAt":"2026-08-11T00:00:00.000Z","smoke":{"cmd":"true","exitCode":0}}]}}}
