# SPDX-FileCopyrightText: Copyright 2021 SeaDve
# SPDX-License-Identifier: GPL-3.0-or-later

from gi.repository import Gst, GObject


class SongPlayer(GObject.GObject):
    __gtype_name__ = 'SongPlayer'
    __gsignals__ = {'stopped': (GObject.SIGNAL_RUN_LAST, None, (str,))}

    _state = Gst.State.NULL

    def __init__(self):
        super().__init__()
        self.playbin = Gst.ElementFactory.make('playbin')

    @GObject.Property(type=Gst.State, default=_state)
    def state(self):
        return self._state

    @state.setter  # type: ignore
    def state(self, playbin_state):
        self._state = playbin_state
        self.playbin.set_state(playbin_state)

    def play(self, song_src):
        self.play_bus = self.playbin.get_bus()
        self.play_bus.add_signal_watch()
        self.handler_id = self.play_bus.connect('message', self._on_gst_message)
        self.playbin.set_property('uri', song_src)
        self.props.state = Gst.State.PLAYING

    def stop(self):
        self.props.state = Gst.State.NULL
        self.play_bus.remove_watch()
        self.play_bus.disconnect(self.handler_id)
        self.emit('stopped', self.playbin.get_property('uri'))

    def _on_gst_message(self, bus, message):
        t = message.type
        if t == Gst.MessageType.ERROR:
            err, debug = message.parse_error()
            print("Error: %s" % err, debug)
        elif t == Gst.MessageType.EOS:
            self.stop()
        elif t == Gst.MessageType.BUFFERING:
            percent = message.parse_buffering()
            if percent < 100:
                self.props.state = Gst.State.PAUSED
            else:
                self.props.state = Gst.State.PLAYING
        elif t == Gst.MessageType.CLOCK_LOST:
            self.props.state = Gst.State.PAUSED
            self.props.state = Gst.State.PLAYING
