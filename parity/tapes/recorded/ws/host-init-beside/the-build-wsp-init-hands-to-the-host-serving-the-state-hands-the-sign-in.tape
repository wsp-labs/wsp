{"meta":{"door":"ws","source":"packages/host/test/init-beside.test.ts > the build wsp init hands to the host serving the state > hands the sign-in's page over and types the code that page asked for back into the machine; a stop from the terminal cancels the job","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8"}}
{"in":{"id":2,"op":"events.subscribe"}}
{"out":{"id":2,"ok":true,"seq":0,"stream":"00000000-0000-4000-8000-000000000001"}}
{"in":{"road":"terminal","id":3,"op":"init.start"}}
{"out":{"id":3,"ok":true,"job":{"id":"init_1","road":"terminal","keys":{"solari":true},"step":0,"stoppable":true,"screens":[],"progress":{"done":0,"total":1},"log":[],"phase":"reading","rows":[]}}}
{"out":{"type":"init.job","job":{"id":"init_1","road":"terminal","keys":{"solari":true},"step":0,"stoppable":true,"screens":[],"progress":{"done":0,"total":1},"log":[],"phase":"answering","rows":[]}}}
{"in":{"id":4,"op":"init.build"}}
{"out":{"id":4,"ok":true,"job":{"id":"init_1","road":"terminal","keys":{"solari":true},"step":0,"stoppable":true,"screens":[],"progress":{"done":0,"total":1},"log":[],"phase":"answering","rows":[]}}}
{"out":{"type":"init.job","job":{"id":"init_1","road":"terminal","keys":{"solari":true},"step":0,"stoppable":true,"screens":[],"progress":{"done":0,"total":1},"log":[],"phase":"signing-in","rows":[{"id":"sign-in/gh","kind":"sign-in","tool":"gh","label":"GitHub CLI login","state":"waiting for you","page":"https://github.com/login/device","finish":"code"}]}}}
{"in":{"tool":"gh","code":"<redacted-1>","id":5,"op":"init.signInCode"}}
{"out":{"id":5,"ok":true,"job":{"id":"init_1","road":"terminal","keys":{"solari":true},"step":0,"stoppable":true,"screens":[],"progress":{"done":0,"total":1},"log":[],"phase":"signing-in","rows":[{"id":"sign-in/gh","kind":"sign-in","tool":"gh","label":"GitHub CLI login","state":"waiting for you","page":"https://github.com/login/device","finish":"code"}]}}}
{"in":{"id":6,"op":"init.cancel"}}
{"out":{"type":"init.job","job":{"id":"init_1","road":"terminal","keys":{"solari":true},"step":0,"stoppable":true,"screens":[],"progress":{"done":0,"total":1},"log":[],"phase":"cancelled","rows":[{"id":"sign-in/gh","kind":"sign-in","tool":"gh","label":"GitHub CLI login","state":"waiting for you","page":"https://github.com/login/device","finish":"code"}],"error":"You stopped the build"}}}
{"out":{"id":6,"ok":true,"job":{"id":"init_1","road":"terminal","keys":{"solari":true},"step":0,"stoppable":true,"screens":[],"progress":{"done":0,"total":1},"log":[],"phase":"cancelled","rows":[{"id":"sign-in/gh","kind":"sign-in","tool":"gh","label":"GitHub CLI login","state":"waiting for you","page":"https://github.com/login/device","finish":"code"}],"error":"You stopped the build"}}}
