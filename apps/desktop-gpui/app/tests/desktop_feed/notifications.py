# SPDX-License-Identifier: AGPL-3.0-only
# A stand-in notification server on the session bus: owns org.freedesktop.Notifications, writes each Notify it is
# sent to the file named first, and answers it with a click (ActionInvoked "default") half a second later, or with
# nothing when the second argument is --hold, as a desktop that keeps every line in its tray until it is cleared.
import sys
import dbus
import dbus.service
import dbus.mainloop.glib
from gi.repository import GLib

LOG = sys.argv[1]
HOLD = sys.argv[2:] == ["--hold"]
IFACE = "org.freedesktop.Notifications"


class Server(dbus.service.Object):
    def __init__(self, bus):
        super().__init__(bus, "/org/freedesktop/Notifications")
        self.next = 0

    @dbus.service.method(IFACE, in_signature="susssasa{sv}i", out_signature="u")
    def Notify(self, app, replaces, icon, summary, body, actions, hints, timeout):
        self.next += 1
        with open(LOG, "a") as f:
            f.write(f"Notify app={app!s} summary={summary!s} body={body!s} actions={list(map(str, actions))}\n")
        nid = self.next
        if not HOLD:
            GLib.timeout_add(500, lambda: (self.ActionInvoked(nid, "default"), False)[1])
        return nid

    @dbus.service.method(IFACE, in_signature="", out_signature="as")
    def GetCapabilities(self):
        return ["actions", "body"]

    @dbus.service.method(IFACE, in_signature="", out_signature="ssss")
    def GetServerInformation(self):
        return ("stand-in", "wsp", "1", "1.2")

    @dbus.service.method(IFACE, in_signature="u", out_signature="")
    def CloseNotification(self, nid):
        self.NotificationClosed(nid, 3)

    @dbus.service.signal(IFACE, signature="us")
    def ActionInvoked(self, nid, key):
        with open(LOG, "a") as f:
            f.write(f"ActionInvoked {nid} {key}\n")

    @dbus.service.signal(IFACE, signature="uu")
    def NotificationClosed(self, nid, reason):
        pass


dbus.mainloop.glib.DBusGMainLoop(set_as_default=True)
bus = dbus.SessionBus()
name = dbus.service.BusName(IFACE, bus)
server = Server(bus)
with open(LOG, "a") as f:
    f.write("ready\n")
GLib.MainLoop().run()
