<h1 align="center">
  <img src="data/logo/io.github.seadve.Mousai.svg" alt="Mousai" width="192" height="192"/><br>
  Mousai
</h1>

<p align="center"><strong>Simple song identifier</strong></p>

<p align="center">
  <a href="https://flathub.org/apps/details/io.github.seadve.Mousai"><img width="200" alt="Download on Flathub" src="https://flathub.org/assets/badges/flathub-badge-en.png"/></a>
</p>

<br>
<p align="center">
  <a href="https://hosted.weblate.org/engage/kooha/">
    <img src="https://hosted.weblate.org/widgets/kooha/-/mousai/svg-badge.svg" alt="Translation status"  />
  </a>
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

## AudD

You can check their [Privacy Policy](https://audd.io/privacy/) and [Terms of Services](https://audd.io/terms/) for more informations about AudD.


## Credits

Developed by **[Dave Patrick](https://github.com/SeaDve)**.
