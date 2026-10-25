{"meta":{"door":"ws","source":"packages/runtime/test/project-golden.test.ts > a project golden > over the wire: workspaces.snapshot replies with the project golden, projectGoldens.list with every one, and a refusal carries its kind","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"workspaceId":"ws_00000001","id":2,"op":"workspaces.snapshot"}}
{"out":{"id":2,"ok":true,"projectGolden":{"snapshotId":"snap_wsp-h1s1-project-proj-2026-09-06T10-01-00-000Z","projects":[{"name":"proj","dest":"/root/proj","importedAt":"2026-09-06T10:00:00.000Z"}],"golden":"snap_golden-v12","version":12,"workspaceId":"ws_00000001","workspaceName":"task","createdAt":"2026-09-06T10:01:00.000Z"}}}
{"in":{"id":3,"op":"projectGoldens.list"}}
{"out":{"id":3,"ok":true,"projectGoldens":[{"snapshotId":"snap_wsp-h1s1-project-proj-2026-09-06T10-01-00-000Z","projects":[{"name":"proj","dest":"/root/proj","importedAt":"2026-09-06T10:00:00.000Z"}],"golden":"snap_golden-v12","version":12,"workspaceId":"ws_00000001","workspaceName":"task","createdAt":"2026-09-06T10:01:00.000Z"}]}}
{"in":{"workspaceId":"ws_00000001","id":4,"op":"workspaces.snapshot"}}
{"out":{"id":4,"ok":false,"error":"snapshot wsp-h1s1-project-proj-2026-09-06T10-01-00-000Z refused: machine m1 is not first-life (it was resumed); snapshots only come from fresh machines","kind":"notFirstLife"}}
