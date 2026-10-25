{"meta":{"door":"ws","source":"packages/runtime/test/places-folders.test.ts > the folders of a computer you own > are read by that computer's daemon over its link, told the folder of every project recorded there, and answered as it listed them","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"on":"p_00000001","dir":"/home/maya","hidden":true,"id":2,"op":"host.folders"}}
{"out":{"id":2,"ok":true,"listing":{"dir":"/home/maya","roots":["/home/maya","/wsp/projects/p_1/checkout"],"folders":[{"path":"/home/maya/code","repo":true}],"hidden":2}}}
{"in":{"on":"p_00000001","repos":true,"id":3,"op":"host.folders"}}
{"out":{"id":3,"ok":true,"listing":{"dir":"/home/maya","roots":["/home/maya","/wsp/projects/p_1/checkout"],"folders":[{"path":"/home/maya/code","repo":true}],"hidden":2}}}
