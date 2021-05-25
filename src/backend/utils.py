# utils.py
#
# Copyright 2021 SeaDve
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program.  If not, see <http://www.gnu.org/licenses/>.

import json
import requests
import re
import urllib.request

from gi.repository import GLib


class Utils:

    @staticmethod
    def _simplify(res):
        status = res['status']
        output = {'status': status}
        image_src = None
        if status == 'success':
            if song_result := res['result']:
                song_link = song_result['song_link']
                output['result'] = {
                    'title': song_result['title'],
                    'artist': song_result['artist'],
                    'song_link': song_link,
                    'audio_src': Utils._get_audio_src(song_link)
                }
                image_src = Utils._get_image_src(song_result)
            else:
                output['result'] = None
        else:
            output['error_message'] = res['error']['error_message']
        return output, image_src

    @staticmethod
    def _get_audio_src(url):
        try:
            page = requests.get(url)
            track = re.findall(r'tracks = .*;', page.text)[0] \
                .replace('tracks = ', '') \
                .replace(';', '')
            track = json.loads(track)[0]['sample']['src']
            return track
        except Exception as e:
            print(e)
            return ''

    @staticmethod
    def _get_image_src(res):
        try:
            return res['spotify']['album']['images'][2]['url']
        except KeyError:
            return ''

    @staticmethod
    def download_image(link, save_dir):
        urllib.request.urlretrieve(link, save_dir)

    @staticmethod
    def guess_song(song_file, token):
        data = {'api_token': token, 'return': 'spotify'}
        files = {'file': open(song_file, 'rb')}
        try:
            res = requests.post('https://api.audd.io/', data=data, files=files).json()
        except Exception as error:
            res = {'status': 'error', 'error': {'error_message': f"Connection Error:{error}"}}
        finally:
            return Utils._simplify(res)

    @staticmethod
    def get_tmp_dir():
        directory = GLib.getenv('XDG_CACHE_HOME')
        if not directory:
            directory = ''
        return f'{directory}/tmp'
