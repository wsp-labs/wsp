{"meta":{"door":"ws","source":"packages/runtime/test/project-import.test.ts > project.import on a workspace > over the wire the host's bundler answers project.plan and feeds project.import; a server without one refuses both","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"workspaceId":"ws_00000001","source":"/Users/dev/code/proj","dest":"/root/other","id":2,"op":"project.import"}}
{"out":{"id":2,"ok":false,"error":"this runtime cannot read folders on this computer"}}
