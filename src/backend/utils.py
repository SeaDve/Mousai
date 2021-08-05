# SPDX-FileCopyrightText: Copyright 2021 SeaDve
# SPDX-License-Identifier: GPL-3.0-or-later

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
        if not Utils.check_has_audio(song_file):
            return Utils._simplify({
                'status': 'error', 'error': {
                    'error_message': "No audio detected. Your microphone may "
                                     "be disconnected or not detected."
                }
            })

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
        directory = GLib.get_user_cache_dir()
        if not directory:
            directory = ''
        return f'{directory}/tmp'

    @staticmethod
    def check_has_audio(filename):
        import subprocess

        result = subprocess.run(
            [
                "ffprobe", "-v", "error", "-show_entries", "format=nb_streams",
                "-of", "default=noprint_wrappers=1:nokey=1", filename
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT
        )

        try:
            return int(result.stdout) - 1
        except ValueError:
            return False
