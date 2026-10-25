{"meta":{"door":"ws","source":"packages/host/test/verbs-threads.test.ts > wsp verbs over the host: threads, projects and agent defaults > a line that takes something away answers the host's own refusal before it asks anything","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"init"}}
{"in":{"ref":"stub-1","id":2,"op":"projects.resolve"}}
{"out":{"id":2,"ok":true,"project":{"id":"pr_00000001","name":"stub-1","computer":"default","source":{"kind":"git","url":"https://github.com/wsp/stub-1.git"},"path":"/root/stub-1","remote":"https://github.com/wsp/stub-1.git","defaultBranch":"main","memoryKey":"-root-stub-1","memoryDir":"/root/.claude-cfg/projects/-root-stub-1/memory","createdAt":"2026-10-25T15:21:30.725Z"}}}
{"in":{"projectId":"pr_00000001","check":true,"id":3,"op":"projects.remove"}}
{"out":{"id":3,"ok":false,"error":"stub-1 has a workspace standing on it: alpha; delete its threads first","kind":"conflict"}}
