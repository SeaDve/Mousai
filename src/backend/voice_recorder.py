# SPDX-FileCopyrightText: Copyright 2021 SeaDve
# SPDX-License-Identifier: GPL-3.0-or-later

import subprocess

import gi
gi.require_version('GstPbutils', '1.0')
from gi.repository import Gst, GstPbutils, GObject

from mousai.backend.utils import Utils
from mousai.backend.timer import Timer


class VoiceRecorder(GObject.GObject):
    __gsignals__ = {'record-done': (GObject.SIGNAL_RUN_LAST, None, ())}

    peak = GObject.Property(type=float)

    def __init__(self):
        super().__init__()

        self.pipeline = Gst.Pipeline()
        self.src = Gst.ElementFactory.make('pulsesrc')
        audio_convert = Gst.ElementFactory.make('audioconvert')
        caps = Gst.Caps.from_string('audio/x-raw')
        self.level = Gst.ElementFactory.make('level')
        self.encodebin = Gst.ElementFactory.make('encodebin')
        self.filesink = Gst.ElementFactory.make('filesink')

        self.pipeline.add(self.src)
        self.pipeline.add(audio_convert)
        self.pipeline.add(self.level)
        self.pipeline.add(self.encodebin)
        self.pipeline.add(self.filesink)

        self.src.link(audio_convert)
        audio_convert.link_filtered(self.level, caps)

    def start(self):
        self.record_bus = self.pipeline.get_bus()
        self.record_bus.add_signal_watch()
        self.handler_id = self.record_bus.connect('message', self._on_gst_message)

        self.src.set_property('device', self.get_default_audio_input())
        self.encodebin.set_property('profile', self.get_profile())
        self.filesink.set_property('location', f'{Utils.get_tmp_dir()}/mousaitmp.ogg')
        self.level.link(self.encodebin)
        self.encodebin.link(self.filesink)

        self.pipeline.set_state(Gst.State.PLAYING)

        self.timer = Timer(self.stop)
        self.timer.start(5)

    def cancel(self):
        self.pipeline.set_state(Gst.State.NULL)
        self.record_bus.remove_watch()
        self.record_bus.disconnect(self.handler_id)
        self.timer.cancel()

    def stop(self):
        self.pipeline.set_state(Gst.State.NULL)
        self.record_bus.remove_watch()
        self.record_bus.disconnect(self.handler_id)
        self.emit('record-done')

    def _on_gst_message(self, bus, message):
        t = message.type
        if t == Gst.MessageType.ELEMENT:
            self.peak = message.get_structure().get_value('peak')[0]
        elif t == Gst.MessageType.EOS:
            self.stop()
        elif t == Gst.MessageType.ERROR:
            err, debug = message.parse_error()
            print("Error: %s" % err, debug)

    @staticmethod
    def get_profile():
        audio_caps = Gst.Caps.from_string('audio/x-opus')
        encoding_profile = GstPbutils.EncodingAudioProfile.new(audio_caps, None, None, 1)
        container_caps = Gst.Caps.from_string('application/ogg')
        container_profile = GstPbutils.EncodingContainerProfile.new('record', None,
                                                                    container_caps, None)
        container_profile.add_profile(encoding_profile)
        return container_profile

    @staticmethod
    def get_default_audio_input():
        pactl_output = subprocess.run(
            ['/usr/bin/pactl', 'info'],
            stdout=subprocess.PIPE,
            text=True
        ).stdout.splitlines()
        return pactl_output[13].split()[2]
