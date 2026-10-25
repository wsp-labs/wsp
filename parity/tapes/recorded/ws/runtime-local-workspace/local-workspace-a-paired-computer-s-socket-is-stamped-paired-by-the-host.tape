{"meta":{"door":"ws","source":"packages/runtime/test/local-workspace.test.ts > local workspace > a paired computer's socket is stamped paired by the host, whatever origin its client sends","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"id":2,"op":"pair.issue"}}
{"out":{"id":2,"ok":true,"code":"<redacted>","expiresAt":1792928893623}}
{"in":{"workspaceId":"ws_00000001","argv":["true"],"origin":"relayed","id":3,"op":"workspaces.exec"}}
{"out":{"id":3,"ok":false,"error":"mac is this computer; it answers only requests from this computer, never one relayed from a machine"}}
{"in":{"workspaceId":"ws_00000001","argv":["true"],"id":4,"op":"workspaces.exec"}}
{"out":{"id":4,"ok":true,"execId":"8c61779738f1","cwd":"<run>/wsp-project-000001"}}
{"out":{"type":"exec.exit","execId":"8c61779738f1","exitCode":0}}
