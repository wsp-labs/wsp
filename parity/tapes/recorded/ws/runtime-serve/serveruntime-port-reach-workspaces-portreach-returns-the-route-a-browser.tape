{"meta":{"door":"ws","source":"packages/runtime/test/serve.test.ts > serveRuntime port reach > workspaces.portReach returns the route a browser frames, without the daemon token; an unknown workspace is refused","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"source":"https://github.com/wsp/wire-13.git","on":"default","id":2,"op":"projects.add"}}
{"out":{"id":2,"ok":true,"project":{"id":"pr_00000001","name":"wire-13","computer":"default","source":{"kind":"git","url":"https://github.com/wsp/wire-13.git"},"path":"/root/wire-13","remote":"https://github.com/wsp/wire-13.git","defaultBranch":"main","memoryKey":"-root-wire-13","memoryDir":"/root/.claude-cfg/projects/-root-wire-13/memory","createdAt":"2026-10-25T15:20:16.581Z"}}}
{"in":{"project":"pr_00000001","name":"x","golden":"snap_g","id":3,"op":"workspaces.create"}}
{"out":{"id":3,"ok":true,"workspace":{"id":"ws_00000002","name":"x","machineId":"m1","phase":"running","kind":"cloud","golden":"snap_g","createdAt":"2026-10-25T15:20:16.582Z","project":{"id":"pr_00000001","name":"wire-13","path":"/root/wire-13","computer":"default"},"home":"/root","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}}}
{"in":{"workspaceId":"ws_00000002","port":3000,"id":4,"op":"workspaces.portReach"}}
{"out":{"id":4,"ok":true,"reach":{"url":"https://m1-3000.preview.example/?pt_token=e","expiresAt":1800000000000}}}
{"in":{"workspaceId":"ws_nobody","port":3000,"id":5,"op":"workspaces.portReach"}}
{"out":{"id":5,"ok":false,"error":"no such workspace: ws_nobody","kind":"not-found"}}
