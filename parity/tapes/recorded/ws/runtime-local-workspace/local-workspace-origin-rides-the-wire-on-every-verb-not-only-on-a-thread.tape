{"meta":{"door":"ws","source":"packages/runtime/test/local-workspace.test.ts > local workspace > origin rides the wire on every verb, not only on a thread start","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"workspaceId":"ws_00000001","origin":"relayed","id":2,"op":"workspaces.nap"}}
{"out":{"id":2,"ok":false,"error":"mac is this computer; it answers only requests from this computer, never one relayed from a machine"}}
{"in":{"origin":"relayed","id":3,"op":"workspaces.list"}}
{"out":{"id":3,"ok":true,"workspaces":[]}}
{"in":{"id":4,"op":"workspaces.list"}}
{"out":{"id":4,"ok":true,"workspaces":[{"id":"ws_00000001","name":"mac","machineId":"local","phase":"running","kind":"local","golden":"","createdAt":"2026-10-25T15:20:23.304Z","project":{"id":"pr_00000002","name":"mac","path":"<run>/wsp-project-000001","computer":"here"},"folder":"<run>/wsp-project-000001","home":"<run>/wsp-localws-000002","agents":{"spawn":true,"maxMachines":3,"maxDepth":2}}]}}
{"in":{"workspaceId":"ws_00000001","name":"mini","origin":"relayed","id":5,"op":"workspaces.rename"}}
{"out":{"id":5,"ok":false,"error":"mac is this computer; it answers only requests from this computer, never one relayed from a machine"}}
