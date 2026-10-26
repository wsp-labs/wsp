{"meta":{"door":"ws","source":"packages/runtime/test/places-agents.test.ts > the agents on a computer you own > are refused on a cloud account, on a place nobody holds, on a socket let in on a ticket, and on a runtime with no reader","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"target":{"placeId":"solari"},"id":2,"op":"agents.read"}}
{"out":{"id":2,"ok":false,"error":"Solari keeps no computer to read, since every workspace there is forked fresh from the image; name a workspace there, or edit the image","kind":"usage"}}
{"in":{"target":{"placeId":"pl_nobody"},"id":3,"op":"agents.read"}}
{"out":{"id":3,"ok":false,"error":"no place named pl_nobody; you have zingzys-mac, srv, Solari","kind":"usage"}}
{"in":{"purpose":"connect","id":4,"op":"ticket.issue"}}
{"out":{"id":4,"ok":true,"ticket":"<redacted-1>","expiresAt":1792950735775}}
