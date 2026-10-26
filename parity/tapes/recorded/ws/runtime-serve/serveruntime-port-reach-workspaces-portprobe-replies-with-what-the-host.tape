{"meta":{"door":"ws","source":"packages/runtime/test/serve.test.ts > serveRuntime port reach > workspaces.portProbe replies with what the host saw on the port's route; an unknown workspace is refused","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"source":"https://github.com/wsp/wire-14.git","on":"default","id":2,"op":"projects.add"}}
{"out":{"id":2,"ok":true,"project":{"id":"pr_00000001","name":"wire-14","computer":"default","source":{"kind":"git","url":"https://github.com/wsp/wire-14.git"},"path":"/root/wire-14","remote":"https://github.com/wsp/wire-14.git","defaultBranch":"main","memoryKey":"-root-wire-14","memoryDir":"/root/.claude-cfg/projects/-root-wire-14/memory","createdAt":"2026-10-25T17:45:46.329Z"}}}
{"in":{"project":"pr_00000001","name":"x","golden":"snap_g","id":3,"op":"workspaces.create"}}
{"out":{"id":3,"ok":true,"workspace":{"id":"ws_00000002","name":"x","machineId":"m1","phase":"running","kind":"cloud","golden":"snap_g","createdAt":"2026-10-25T17:45:46.329Z","project":{"id":"pr_00000001","name":"wire-14","path":"/root/wire-14","computer":"default"},"home":"/root","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}}}
{"in":{"workspaceId":"ws_00000002","port":5173,"id":4,"op":"workspaces.portProbe"}}
{"out":{"id":4,"ok":true,"probe":{"status":403,"body":"Blocked request. This host (m1-5173.preview.example) is not allowed. To allow this host, add it to server.allowedHosts"}}}
{"in":{"workspaceId":"ws_nobody","port":5173,"id":5,"op":"workspaces.portProbe"}}
{"out":{"id":5,"ok":false,"error":"no such workspace: ws_nobody","kind":"not-found"}}
