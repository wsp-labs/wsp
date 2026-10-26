{"meta":{"door":"ws","source":"packages/host/test/contract.test.ts > the agent contract on the command line and the tool door > with --json every command-line verb prints JSON alone on stdout and its last object is the one its MCP tool answers with","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"init"}}
{"in":{"ref":"spare","id":2,"op":"projects.resolve"}}
{"out":{"id":2,"ok":true,"project":{"id":"pr_00000001","name":"spare","computer":"default","source":{"kind":"git","url":"https://github.com/dev/spare.git"},"path":"/root/spare","remote":"https://github.com/dev/spare.git","defaultBranch":"main","memoryKey":"-root-spare","memoryDir":"/root/.claude-cfg/projects/-root-spare/memory","createdAt":"2026-10-25T17:45:49.657Z"}}}
{"in":{"projectId":"pr_00000001","check":true,"id":3,"op":"projects.remove"}}
{"out":{"id":3,"ok":true,"said":""}}
{"in":{"projectId":"pr_00000001","id":4,"op":"projects.remove"}}
{"out":{"id":4,"ok":true,"said":"spare is no longer a project on default; nothing of it was held there"}}
