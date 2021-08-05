# SPDX-FileCopyrightText: Copyright 2021 SeaDve
# SPDX-License-Identifier: GPL-3.0-or-later

from gi.repository import GLib


class Timer:
    def __init__(self, function):
        self.function = function
        self.cancelled = False

    def _update_time(self):
        if self.time_delay == 10 or self.cancelled:
            if not self.cancelled:
                self.function()
            return False
        self.time_delay -= 10
        return True

    def start(self, time_delay):
        self.time_delay = time_delay * 100
        GLib.timeout_add(100, self._update_time)

    def cancel(self):
        self.cancelled = True
