# SPDX-License-Identifier: AGPL-3.0-only
# A stand-in desktop portal on the session bus: owns org.freedesktop.portal.Desktop with the Inhibit interface alone,
# writes each Inhibit and each release to the file named first, and grants every one.
import sys
import dbus
import dbus.service
import dbus.mainloop.glib
from gi.repository import GLib

LOG = sys.argv[1]
INHIBIT = "org.freedesktop.portal.Inhibit"
PROPS = "org.freedesktop.DBus.Properties"


def log(line):
    with open(LOG, "a") as f:
        f.write(line + "\n")


class Request(dbus.service.Object):
    @dbus.service.method("org.freedesktop.portal.Request", in_signature="", out_signature="")
    def Close(self):
        log("Close " + self.__dbus_object_path__)
        self.remove_from_connection()

    @dbus.service.signal("org.freedesktop.portal.Request", signature="ua{sv}")
    def Response(self, response, results):
        pass


class Portal(dbus.service.Object):
    def __init__(self, bus):
        super().__init__(bus, "/org/freedesktop/portal/desktop")
        self.bus = bus

    @dbus.service.method(PROPS, in_signature="ss", out_signature="v")
    def Get(self, iface, prop):
        return dbus.UInt32(3)

    @dbus.service.method(PROPS, in_signature="s", out_signature="a{sv}")
    def GetAll(self, iface):
        return {"version": dbus.UInt32(3)} if iface == INHIBIT else {}

    @dbus.service.method(INHIBIT, in_signature="sua{sv}", out_signature="o", sender_keyword="sender")
    def Inhibit(self, window, flags, options, sender):
        token = str(options.get("handle_token", "t"))
        path = "/org/freedesktop/portal/desktop/request/%s/%s" % (sender[1:].replace(".", "_"), token)
        log("Inhibit flags=%d reason=%s" % (flags, options.get("reason", "")))
        request = Request(self.bus, path)
        GLib.timeout_add(100, lambda: (request.Response(dbus.UInt32(0), {}), False)[1])
        return dbus.ObjectPath(path)


dbus.mainloop.glib.DBusGMainLoop(set_as_default=True)
bus = dbus.SessionBus()
name = dbus.service.BusName("org.freedesktop.portal.Desktop", bus)
portal = Portal(bus)
log("ready")
GLib.MainLoop().run()
