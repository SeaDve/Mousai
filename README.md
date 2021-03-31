<h1 align="center">
  <img src="data/logo/io.github.seadve.Mousai.svg" alt="Mousai" width="192" height="192"/><br>
  Mousai
</h1>

<p align="center"><strong>Simple song identifier</strong></p>

<br>
<p align="center">
    <a href="https://github.com/SeaDve/Mousai/actions/workflows/testing.yml">
    <img src="https://github.com/SeaDve/Mousai/actions/workflows/testing.yml/badge.svg" alt="CI status"/>
  </a>
  <a href="https://paypal.me/sedve">
    <img src="https://img.shields.io/badge/PayPal-Donate-gray.svg?style=flat&logo=paypal&colorA=0071bb&logoColor=fff" alt="Donate" />
  </a>
</p>

<p align="center">
  <img src="screenshots/Mousai-preview.png" alt="Preview"/>
</p>

## Description
Mousai is a simple application that can identify song like Shazam. It saves the artist, album, and title of the identified song in a JSON file.

Note: This uses the API of audd.io, so it is necessary to login to their site to get more trials.


## Building from source

### GNOME Builder (Recommended)
GNOME Builder is the environment used for developing this application. It can use Flatpak manifests to create a consistent building and running environment cross-distro. Thus, it is highly recommended you use it.

1. Download [GNOME Builder](https://flathub.org/apps/details/org.gnome.Builder).
2. In Builder, click the "Clone Repository" button at the bottom, using `https://github.com/SeaDve/Mousai.git` as the URL.
3. Click the build button at the top once the project is loaded.

### Manual with meson
```
git clone https://github.com/SeaDve/Mousai.git
cd Mousai
meson builddir --prefix=/usr/local
ninja -C builddir install
```


## Credits

Developed by **[Dave Patrick](https://github.com/SeaDve)**.
