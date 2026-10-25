{"meta":{"door":"ws","source":"packages/runtime/test/pairing.test.ts > the device listing > lets a paired device revoke another device and cuts that device's socket, as it lets it revoke itself","outs":"recorded"}}
{"in":{"code":"<redacted-1>","name":"three","id":1,"op":"pair.redeem"}}
{"out":{"id":1,"ok":true,"deviceId":"d_00000001","deviceToken":"<redacted-2>"}}
{"in":{"deviceId":"d_00000001","id":2,"op":"devices.revoke"}}
{"out":{"id":2,"ok":true,"revoked":true}}
