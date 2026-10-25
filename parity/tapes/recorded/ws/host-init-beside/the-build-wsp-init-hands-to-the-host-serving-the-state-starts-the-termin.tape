{"meta":{"door":"ws","source":"packages/host/test/init-beside.test.ts > the build wsp init hands to the host serving the state > starts the terminal road, asks for the build, prints every row as it lands and ends on the golden that host recorded","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8"}}
{"in":{"id":2,"op":"events.subscribe"}}
{"out":{"id":2,"ok":true,"seq":0,"stream":"00000000-0000-4000-8000-000000000001"}}
{"in":{"road":"terminal","id":3,"op":"init.start"}}
{"out":{"id":3,"ok":true,"job":{"id":"init_1","road":"terminal","keys":{"solari":true},"step":0,"stoppable":true,"screens":[],"progress":{"done":0,"total":1},"log":[],"phase":"reading","rows":[]}}}
{"out":{"type":"init.job","job":{"id":"init_1","road":"terminal","keys":{"solari":true},"step":0,"stoppable":true,"screens":[],"progress":{"done":0,"total":1},"log":[],"phase":"answering","rows":[]}}}
{"in":{"firstWorkspace":"beside","importFolder":"/Users/maya/spoo","yes":true,"on":"spoo","id":4,"op":"init.build"}}
{"out":{"id":4,"ok":true,"job":{"id":"init_1","road":"terminal","keys":{"solari":true},"step":0,"stoppable":true,"screens":[],"progress":{"done":0,"total":1},"log":[],"phase":"answering","rows":[]}}}
{"out":{"type":"init.job","job":{"id":"init_1","road":"terminal","keys":{"solari":true},"step":0,"stoppable":true,"screens":[],"progress":{"done":0,"total":1},"log":[],"phase":"building","rows":[{"id":"stage/creating","kind":"stage","label":"Machine created","state":"running"}]}}}
{"out":{"type":"init.job","job":{"id":"init_1","road":"terminal","keys":{"solari":true},"step":0,"stoppable":true,"screens":[],"progress":{"done":0,"total":1},"log":[],"phase":"building","rows":[{"id":"stage/creating","kind":"stage","label":"Machine created","state":"waiting for a machine slot"}]}}}
{"out":{"type":"init.job","job":{"id":"init_1","road":"terminal","keys":{"solari":true},"step":0,"stoppable":true,"screens":[],"progress":{"done":0,"total":1},"log":[],"phase":"building","rows":[{"id":"stage/creating","kind":"stage","label":"Machine created","state":"done","detail":"sandbox from ubuntu:24.04","ms":332}]}}}
{"out":{"type":"init.job","job":{"id":"init_1","road":"terminal","keys":{"solari":true},"step":0,"stoppable":true,"screens":[],"progress":{"done":0,"total":1},"log":[],"phase":"done","rows":[{"id":"stage/creating","kind":"stage","label":"Machine created","state":"done","detail":"sandbox from ubuntu:24.04","ms":332},{"id":"stage/installing-tools","kind":"stage","label":"Tools installed","state":"skipped"},{"id":"workspace/beside","kind":"workspace","label":"beside","state":"forked"}],"golden":{"version":1},"workspace":{"id":"ws_1","name":"beside"}}}}
{"in":{"here":true,"id":5,"op":"pair.issue"}}
{"out":{"id":5,"ok":true,"code":"<redacted>","expiresAt":1792929155910}}
{"in":{"id":6,"op":"devices.list"}}
{"out":{"id":6,"ok":true,"devices":[{"id":"d_00000001","name":"a Mac in a browser","createdAt":"2026-10-25T11:42:35.922Z","lastSeenAt":"2026-10-25T11:42:35.922Z","here":true}]}}
