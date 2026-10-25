{"meta":{"door":"ws","source":"packages/host/test/mcp.test.ts > the MCP server over the host > wsp mcp speaks the protocol over stdio and returns when its stdin ends, closing the host socket","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"init"}}
{"in":{"id":2,"op":"projects.list"}}
{"out":{"id":2,"ok":true,"projects":[{"id":"pr_00000001","name":"stub-10","computer":"default","source":{"kind":"git","url":"https://github.com/wsp/stub-10.git"},"path":"/root/stub-10","remote":"https://github.com/wsp/stub-10.git","defaultBranch":"main","memoryKey":"-root-stub-10","memoryDir":"/root/.claude-cfg/projects/-root-stub-10/memory","createdAt":"2026-10-25T11:38:08.304Z"}]}}
{"in":{"id":3,"op":"projects.defaults"}}
{"out":{"id":3,"ok":true,"defaults":{"pr_00000001":{"agent":{"value":"claude","from":"catalog"},"model":{"value":"claude-opus-5-5","from":"catalog"},"effort":{"value":"high","from":"catalog"},"access":{"value":"full","mode":"bypassPermissions","from":"catalog"}}}}}
