{"meta":{"door":"ws","source":"packages/runtime/test/serve.test.ts > serveRuntime snapshot lineage > snapshots.list is the manifest with head; rollback moves head, says existing workspaces are untouched, persists","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"id":2,"op":"snapshots.list"}}
{"out":{"id":2,"ok":true,"lineage":{"name":"default","head":2,"versions":[{"version":1,"snapshotId":"snap_golden-v1","baseTemplate":"base","setupSha":"sha1","createdAt":"2026-08-11T00:00:00.000Z","smoke":{"cmd":"true","exitCode":0}},{"version":2,"snapshotId":"snap_golden-v2","baseTemplate":"base","setupSha":"sha2","createdAt":"2026-08-12T00:00:00.000Z","smoke":{"cmd":"true","exitCode":0}}]}}}
{"in":{"version":1,"id":3,"op":"snapshots.rollback"}}
{"out":{"id":3,"ok":true,"lineage":{"name":"default","head":1,"versions":[{"version":1,"snapshotId":"snap_golden-v1","baseTemplate":"base","setupSha":"sha1","createdAt":"2026-08-11T00:00:00.000Z","smoke":{"cmd":"true","exitCode":0}},{"version":2,"snapshotId":"snap_golden-v2","baseTemplate":"base","setupSha":"sha2","createdAt":"2026-08-12T00:00:00.000Z","smoke":{"cmd":"true","exitCode":0}}]},"existingWorkspaces":"untouched"}}
{"in":{"name":"default","id":4,"op":"golden.get"}}
{"out":{"id":4,"ok":true,"manifest":{"head":1,"versions":[{"version":1,"snapshotId":"snap_golden-v1","baseTemplate":"base","setupSha":"sha1","createdAt":"2026-08-11T00:00:00.000Z","smoke":{"cmd":"true","exitCode":0}},{"version":2,"snapshotId":"snap_golden-v2","baseTemplate":"base","setupSha":"sha2","createdAt":"2026-08-12T00:00:00.000Z","smoke":{"cmd":"true","exitCode":0}}]}}}
{"in":{"name":"other","id":5,"op":"snapshots.list"}}
{"out":{"id":5,"ok":true,"lineage":{"name":"other","head":null,"versions":[]}}}
