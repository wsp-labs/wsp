{"meta":{"door":"ws","source":"packages/runtime/test/thread-defaults.test.ts > an agent's setup on a computer > is the host's own road over the wire, and a value crosses only on a socket holding the host's own token","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"placeId":"here","agent":"claude","env":{"FOO":"<redacted>"},"id":2,"op":"agents.setup"}}
{"out":{"id":2,"ok":true,"agent":{"id":"claude","name":"Claude Code","installed":true,"version":"1.0.0","road":"own","signIn":"signed-in","signInRoad":"device","wspTools":false,"setup":{"on":true,"envNames":["FOO"]}}}}
{"in":{"purpose":"relay","id":3,"op":"ticket.issue"}}
{"out":{"id":3,"ok":true,"ticket":"<redacted>","expiresAt":1792942068601}}
