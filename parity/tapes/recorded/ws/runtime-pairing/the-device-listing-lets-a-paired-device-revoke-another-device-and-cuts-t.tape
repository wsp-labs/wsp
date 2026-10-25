{"meta":{"door":"ws","source":"packages/runtime/test/pairing.test.ts > the device listing > lets a paired device revoke another device and cuts that device's socket, as it lets it revoke itself","outs":"recorded"}}
{"in":{"code":"<redacted-1>","name":"one","id":1,"op":"pair.redeem"}}
{"out":{"id":1,"ok":true,"deviceId":"d_00000001","deviceToken":"<redacted-2>"}}
{"in":{"deviceId":"d_00000002","id":2,"op":"devices.revoke"}}
{"out":{"id":2,"ok":true,"revoked":true}}
{"in":{"deviceId":"d_nope","id":3,"op":"devices.revoke"}}
{"out":{"id":3,"ok":true,"revoked":false}}
{"in":{"id":4,"op":"devices.list"}}
{"out":{"id":4,"ok":true,"devices":[{"id":"d_00000001","name":"one","createdAt":"2026-10-25T17:47:34.318Z","lastSeenAt":"2026-10-25T17:47:34.318Z"}]}}
