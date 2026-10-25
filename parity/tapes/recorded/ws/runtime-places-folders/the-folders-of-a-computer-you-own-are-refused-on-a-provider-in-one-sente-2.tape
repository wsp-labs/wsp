{"meta":{"door":"ws","source":"packages/runtime/test/places-folders.test.ts > the folders of a computer you own > are refused on a provider in one sentence saying what to do instead, on a place nobody holds by the places there are, and on a computer that is not connected by its absent sentence","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"on":"solari","id":2,"op":"host.folders"}}
{"out":{"id":2,"ok":false,"error":"Solari keeps no computer to browse, since every workspace there is forked fresh; add the project from a repository address, or from a folder on this computer, which seeds it","kind":"usage"}}
{"in":{"on":"pl_nobody","id":3,"op":"host.folders"}}
{"out":{"id":3,"ok":false,"error":"no place named pl_nobody; you have zingzys-mac, srv, Solari","kind":"usage"}}
{"in":{"on":"p_00000001","id":4,"op":"host.folders"}}
{"out":{"id":4,"ok":false,"error":"srv is not answering; it connects on its own when it is on"}}
