from gi.repository import GLib, Gst, GObject, Gtk


@Gtk.Template(resource_path='/io/github/seadve/Mousai/ui/button_player.ui')
class ButtonPlayer(Gtk.Button):
    __gtype_name__ = 'ButtonPlayer'

    is_stopped = GObject.Property(type=bool, default=False, flags=GObject.ParamFlags.READWRITE)

    def __init__(self):
        super().__init__()
        self.install_property_action('but.play', 'is_stopped')
        self.playbin = Gst.ElementFactory.make('playbin')
        self.connect('notify::is-stopped', self._on_stopped_notify)

    def _on_stopped_notify(self, but, is_stopped):
        if but.is_stopped:
            self._play()
        else:
            self._stop()

    def _play(self):
        self.play_bus = self.playbin.get_bus()
        self.play_bus.add_signal_watch()
        self.handler_id = self.play_bus.connect('message', self._on_gst_message)
        self.playbin.set_state(Gst.State.PLAYING)

    def _stop(self):
        self.playbin.set_state(Gst.State.NULL)
        self.play_bus.remove_watch()
        self.play_bus.disconnect(self.handler_id)

    def _on_gst_message(self, bus, message):
        t = message.type
        if t == Gst.MessageType.ERROR:
            err, debug = message.parse_error()
            print("Error: %s" % err, debug)
        elif t == Gst.MessageType.EOS:
            self._stop()
        elif t == Gst.MessageType.BUFFERING:
            percent = message.parse_buffering()
            if percent < 100:
                self.playbin.set_state(Gst.State.PAUSED)
            else:
                self.playbin.set_state(Gst.State.PLAYING)
        elif t == Gst.MessageType.CLOCK_LOST:
            self.playbin.set_state(Gst.State.PAUSED)
            self.playbin.set_state(Gst.State.PLAYING)

    def set_song_src(self, song_src):
        self.playbin.set_property('uri', song_src)
