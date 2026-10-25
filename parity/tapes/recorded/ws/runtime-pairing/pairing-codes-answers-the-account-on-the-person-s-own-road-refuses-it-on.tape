{"meta":{"door":"ws","source":"packages/runtime/test/pairing.test.ts > pairing codes > answers the account on the person's own road, refuses it on a ticket, and refuses it on a host keeping no records","outs":"recorded"}}
{"in":{"token":"<<host-token>>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"id":2,"op":"account.get"}}
{"out":{"id":2,"ok":true,"account":{"signedIn":true,"login":"zingzy"}}}
{"in":{"purpose":"relay","id":3,"op":"ticket.issue"}}
{"out":{"id":3,"ok":true,"ticket":"<redacted>","expiresAt":1792928727286}}
