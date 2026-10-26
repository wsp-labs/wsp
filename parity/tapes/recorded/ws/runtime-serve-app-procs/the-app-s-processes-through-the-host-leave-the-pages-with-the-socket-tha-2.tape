{"meta":{"door":"ws","source":"packages/runtime/test/serve-app-procs.test.ts > the app's processes through the host > leave the pages with the socket that sent them, and come only from one that offered","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"procs":[{"pid":501,"kind":"main"},{"pid":502,"kind":"gpu"},{"pid":503,"kind":"network"},{"pid":504,"kind":"renderer"},{"pid":505,"kind":"tab","origins":["https://example.com"]}],"id":2,"op":"appProcs.set"}}
{"out":{"id":2,"ok":false,"error":"appProcs.set comes after appProcs.offer on the same socket"}}
{"in":{"id":3,"op":"appProcs.offer"}}
{"out":{"id":3,"ok":true,"wanted":true}}
{"in":{"procs":[{"pid":501,"kind":"main"},{"pid":502,"kind":"gpu"},{"pid":503,"kind":"network"},{"pid":504,"kind":"renderer"},{"pid":505,"kind":"tab","origins":["https://example.com"]}],"id":4,"op":"appProcs.set"}}
{"out":{"id":4,"ok":true}}
