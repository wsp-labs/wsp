{"meta":{"door":"ws","source":"packages/runtime/test/preferences.test.ts > preferences over the wire > reads as the defaults, takes a patch, keeps it for the next runtime on the store, and pushes the record to every socket","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"id":2,"op":"events.subscribe"}}
{"out":{"id":2,"ok":true,"seq":0,"stream":"00000000-0000-4000-8000-000000000001"}}
{"out":{"type":"preferences.changed","preferences":{"theme":"light","lightTheme":"paper","darkTheme":"graphite","sidebarMode":"list","terminalSize":"app","terminalZoom":{"ws_a":2},"access":{},"projectLook":{},"projectIcon":{},"computerLook":{},"serverIcons":true,"agentVersions":true,"usageLogs":true,"productUsage":true,"keepAwake":true,"transparency":true,"projectOrder":[],"keybindings":{},"appFont":"","codeFont":"","agentDefaults":{},"projectDefaults":{},"sendWith":"enter","midTurn":"queue","notifyNeeds":"notify-sound","notifyDone":"notify","planAlerts":true,"settleAfter":"2h","askDelete":true,"onQuit":"ask","newThreadIn":"current","labs":false,"sidebarWidth":312},"seq":1}}
