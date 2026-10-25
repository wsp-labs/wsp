{"meta":{"door":"ws","source":"packages/runtime/test/project-import.test.ts > project.import on a workspace > over the wire the host's bundler answers project.plan and feeds project.import; a server without one refuses both","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"workspaceId":"ws_00000001","source":"/Users/dev/code/proj","dest":"/root/proj","carry":["keys/id_00000002"],"rewrite":[".git/config"],"agents":["pi"],"id":2,"op":"project.import"}}
{"out":{"id":2,"ok":true,"imported":{"dest":"/root/proj","files":4,"bytes":4000,"parts":1,"cut":[".env","config/secrets.json"],"rewritten":[".git/config"],"agents":[{"agent":"pi","files":1,"bytes":17,"outcome":"moved"}],"project":{"name":"proj","dest":"/root/proj","importedAt":"2026-10-25T15:22:14.094Z","size":4000}}}}
