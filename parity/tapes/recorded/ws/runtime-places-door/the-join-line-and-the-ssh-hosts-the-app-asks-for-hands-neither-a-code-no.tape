{"meta":{"door":"ws","source":"packages/runtime/test/places-door.test.ts > the join line and the ssh hosts the app asks for > hands neither a code nor the ssh hosts to a paired computer or a relayed socket","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"id":2,"op":"places.sshHosts"}}
{"out":{"id":2,"ok":true,"hosts":[{"alias":"hetzner","hostName":"65.21.4.12","user":"root","from":"config"}]}}
{"in":{"id":3,"op":"pair.issue"}}
{"out":{"id":3,"ok":true,"code":"<redacted>","expiresAt":1792942414912,"hostKey":"SHA256:KrkhQMj+wAamIltgWr5jwkzQOkLZqyeHsicdrE3sEG8"}}
{"in":{"purpose":"relay","id":4,"op":"ticket.issue"}}
{"out":{"id":4,"ok":true,"ticket":"<redacted>","expiresAt":1792942114912}}
