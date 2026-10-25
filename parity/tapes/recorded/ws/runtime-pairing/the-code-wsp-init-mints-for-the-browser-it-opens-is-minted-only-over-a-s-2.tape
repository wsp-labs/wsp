{"meta":{"door":"ws","source":"packages/runtime/test/pairing.test.ts > the code wsp init mints for the browser it opens > is minted only over a socket holding the host's own token: the owner's browser cannot mint one, nor can a laptop","outs":"recorded"}}
{"in":{"code":"<redacted>","name":"a Mac in a browser","id":1,"op":"pair.redeem"}}
{"out":{"id":1,"ok":true,"deviceId":"d_00000001","deviceToken":"EwDtkLnUSICCgD3XikQG14_f28hzLwRd"}}
{"in":{"here":true,"id":2,"op":"pair.issue"}}
{"out":{"id":2,"ok":false,"error":"only a socket holding this host's own token may mint a pairing code; run wsp host pair on the computer the host runs on"}}
{"in":{"id":3,"op":"pair.issue"}}
{"out":{"id":3,"ok":false,"error":"only a socket holding this host's own token may mint a pairing code; run wsp host pair on the computer the host runs on"}}
