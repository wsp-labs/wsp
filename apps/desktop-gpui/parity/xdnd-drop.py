#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Drops files on an X window as a file manager's drag does, over XDND, for an app that takes no other road to them:
#   DISPLAY=:97 python3 xdnd-drop.py <window id> <x> <y> <file>...
# x and y are in the window's own pixels; the pointer is moved there first, since a GPUI window reads the drop's place
# off the pointer. It offers text/uri-list alone, answers the target's request for it, and drops once the target has
# taken the list and said it accepts.
import ctypes
import os
import sys
import time
from urllib.parse import quote

if len(sys.argv) < 5:
    sys.exit("usage: xdnd-drop.py <window id> <x> <y> <file>...")
target = int(sys.argv[1], 0)
x, y = int(sys.argv[2]), int(sys.argv[3])
files = [os.path.abspath(f) for f in sys.argv[4:]]
for f in files:
    if not os.path.isfile(f):
        sys.exit(f"xdnd-drop: no file {f}")
uris = "".join(f"file://{quote(f)}\r\n" for f in files).encode()

X = ctypes.cdll.LoadLibrary("libX11.so.6")
c_ulong, c_long, c_int, c_void_p = ctypes.c_ulong, ctypes.c_long, ctypes.c_int, ctypes.c_void_p


class ClientMessage(ctypes.Structure):
    _fields_ = [("type", c_int), ("serial", c_ulong), ("send_event", c_int), ("display", c_void_p), ("window", c_ulong),
                ("message_type", c_ulong), ("format", c_int), ("l", c_long * 5)]


class SelectionRequest(ctypes.Structure):
    _fields_ = [("type", c_int), ("serial", c_ulong), ("send_event", c_int), ("display", c_void_p), ("owner", c_ulong),
                ("requestor", c_ulong), ("selection", c_ulong), ("target", c_ulong), ("property", c_ulong), ("time", c_ulong)]


class Selection(ctypes.Structure):
    _fields_ = [("type", c_int), ("serial", c_ulong), ("send_event", c_int), ("display", c_void_p), ("requestor", c_ulong),
                ("selection", c_ulong), ("target", c_ulong), ("property", c_ulong), ("time", c_ulong)]


class Event(ctypes.Union):
    _fields_ = [("type", c_int), ("client", ClientMessage), ("request", SelectionRequest), ("selection", Selection), ("pad", c_long * 24)]


CLIENT_MESSAGE, SELECTION_REQUEST, PROP_MODE_REPLACE, ATOM_TYPE = 33, 30, 0, 4
X.XOpenDisplay.restype = c_void_p
X.XOpenDisplay.argtypes = [ctypes.c_char_p]
X.XInternAtom.restype = c_ulong
X.XInternAtom.argtypes = [c_void_p, ctypes.c_char_p, c_int]
X.XDefaultRootWindow.restype = c_ulong
X.XDefaultRootWindow.argtypes = [c_void_p]
X.XCreateSimpleWindow.restype = c_ulong
X.XCreateSimpleWindow.argtypes = [c_void_p, c_ulong, c_int, c_int, ctypes.c_uint, ctypes.c_uint, ctypes.c_uint, c_ulong, c_ulong]
X.XSetSelectionOwner.argtypes = [c_void_p, c_ulong, c_ulong, c_ulong]
X.XGetSelectionOwner.restype = c_ulong
X.XGetSelectionOwner.argtypes = [c_void_p, c_ulong]
X.XSendEvent.argtypes = [c_void_p, c_ulong, c_int, c_long, ctypes.POINTER(Event)]
X.XChangeProperty.argtypes = [c_void_p, c_ulong, c_ulong, c_ulong, c_int, c_int, ctypes.c_char_p, c_int]
X.XWarpPointer.argtypes = [c_void_p, c_ulong, c_ulong, c_int, c_int, ctypes.c_uint, ctypes.c_uint, c_int, c_int]
X.XTranslateCoordinates.argtypes = [c_void_p, c_ulong, c_ulong, c_int, c_int, ctypes.POINTER(c_int), ctypes.POINTER(c_int), ctypes.POINTER(c_ulong)]
X.XPending.argtypes = [c_void_p]
X.XNextEvent.argtypes = [c_void_p, ctypes.POINTER(Event)]
X.XFlush.argtypes = [c_void_p]
X.XSync.argtypes = [c_void_p, c_int]
X.XCloseDisplay.argtypes = [c_void_p]

d = X.XOpenDisplay(None)
if not d:
    sys.exit("xdnd-drop: no X display; set DISPLAY")
atom = lambda name: X.XInternAtom(d, name.encode(), 0)
A = {n: atom(n) for n in ["XdndSelection", "XdndEnter", "XdndPosition", "XdndStatus", "XdndDrop", "XdndFinished", "XdndLeave", "XdndActionCopy", "text/uri-list", "TARGETS"]}
root = X.XDefaultRootWindow(d)
source = X.XCreateSimpleWindow(d, root, 0, 0, 1, 1, 0, 0, 0)
X.XSetSelectionOwner(d, A["XdndSelection"], source, 0)
if X.XGetSelectionOwner(d, A["XdndSelection"]) != source:
    sys.exit("xdnd-drop: could not own XdndSelection")

rx, ry, child = c_int(), c_int(), c_ulong()
X.XTranslateCoordinates(d, target, root, x, y, ctypes.byref(rx), ctypes.byref(ry), ctypes.byref(child))
X.XWarpPointer(d, 0, root, 0, 0, 0, 0, rx.value, ry.value)
X.XSync(d, 0)


def send(kind, *data):
    e = Event()
    e.client.type = CLIENT_MESSAGE
    e.client.window = target
    e.client.message_type = A[kind]
    e.client.format = 32
    for i, v in enumerate(data):
        e.client.l[i] = v
    X.XSendEvent(d, target, 0, 0, ctypes.byref(e))
    X.XFlush(d)


position = lambda: send("XdndPosition", source, 0, (rx.value << 16) | ry.value, 0, A["XdndActionCopy"])
send("XdndEnter", source, 5 << 24, A["text/uri-list"], 0, 0)
position()
served = accepted = dropped = finished = False
until = time.time() + 10
while time.time() < until and not finished:
    if X.XPending(d) == 0:
        time.sleep(0.02)
        if served and accepted and not dropped:
            time.sleep(0.3)
            send("XdndDrop", source, 0, 0)
            dropped = True
        continue
    e = Event()
    X.XNextEvent(d, ctypes.byref(e))
    if e.type == SELECTION_REQUEST:
        r = e.request
        reply = Event()
        reply.selection.type = 31
        reply.selection.requestor, reply.selection.selection, reply.selection.target, reply.selection.time = r.requestor, r.selection, r.target, r.time
        reply.selection.property = r.property
        if r.target == A["text/uri-list"]:
            X.XChangeProperty(d, r.requestor, r.property, r.target, 8, PROP_MODE_REPLACE, uris, len(uris))
            served = True
        elif r.target == A["TARGETS"]:
            targets = (c_ulong * 1)(A["text/uri-list"])
            X.XChangeProperty(d, r.requestor, r.property, ATOM_TYPE, 32, PROP_MODE_REPLACE, ctypes.cast(targets, ctypes.c_char_p), 1)
        else:
            reply.selection.property = 0
        X.XSendEvent(d, r.requestor, 0, 0, ctypes.byref(reply))
        X.XFlush(d)
        if served and not accepted:
            position()
    elif e.type == CLIENT_MESSAGE:
        m = e.client
        if m.message_type == A["XdndStatus"]:
            accepted = accepted or bool(m.l[1] & 1)
        elif m.message_type == A["XdndFinished"]:
            finished = True
X.XCloseDisplay(d)
if not finished:
    sys.exit(f"xdnd-drop: the drop did not finish (list taken: {served}, accepted: {accepted}, dropped: {dropped})")
print(f"xdnd-drop: dropped {len(files)} files on {hex(target)} at {x},{y}")
