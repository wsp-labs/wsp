{"meta":{"door":"ws","source":"packages/runtime/test/serve-app-procs.test.ts > the app's processes through the host > leave the pages with the socket that sent them, and come only from one that offered","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"id":2,"op":"appProcs.watch"}}
{"out":{"id":2,"ok":true,"procs":[]}}
{"out":{"type":"app.procs","procs":[{"pid":501,"kind":"main"},{"pid":502,"kind":"gpu"},{"pid":503,"kind":"network"},{"pid":504,"kind":"renderer"},{"pid":505,"kind":"tab","origins":["https://example.com"]}]}}
{"out":{"type":"app.procs","procs":[]}}
