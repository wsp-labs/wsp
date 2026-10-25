{"meta":{"door":"ws","source":"packages/runtime/test/serve.test.ts > serveRuntime workspaces.exec > replies with an exec id, pushes each output line to the asking socket and the exit code last, and pushes nothing to another socket"}}
{"in":{"token":"<redacted>","id":1,"op":"auth"}}
{"in":{"source":"https://github.com/wsp/wire-17.git","on":"default","id":2,"op":"projects.add"}}
{"in":{"project":"pr_ee3f3d48","name":"x","golden":"snap_g","id":3,"op":"workspaces.create"}}
{"in":{"workspaceId":"ws_66c5a377","argv":["sh","-c","printf 'one\\ntwo\\n'; exit 3"],"id":4,"op":"workspaces.exec"}}
{"in":{"workspaceId":"ws_nope","argv":["true"],"id":5,"op":"workspaces.exec"}}
