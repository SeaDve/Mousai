# SPDX-FileCopyrightText: Copyright 2021 SeaDve
# SPDX-License-Identifier: GPL-3.0-or-later


from gi.repository import GObject


class Song(GObject.GObject):

    title = GObject.Property(type=str)
    artist = GObject.Property(type=str)
    song_link = GObject.Property(type=str)
    song_src = GObject.Property(type=str)

    def __init__(self, title, artist, song_link, song_src=''):
        super().__init__()

        self.title = title
        self.artist = artist
        self.song_link = song_link
        self.song_src = song_src

    def __iter__(self):
        yield 'title', self.title
        yield 'artist', self.artist
        yield 'song_link', self.song_link
        yield 'song_src', self.song_src
