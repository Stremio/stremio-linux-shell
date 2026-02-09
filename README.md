<div align="center">

![Stremio icon](data/icons/com.stremio.Stremio.svg "Stremio icon")

# Stremio on Linux 
Client for Stremio on Linux using [`winit`](https://github.com/rust-windowing/winit) + [`glutin`](https://github.com/rust-windowing/glutin) with [`libmpv`](https://github.com/mpv-player/mpv/blob/master/DOCS/man/libmpv.rst) and [`CEF`](https://github.com/chromiumembedded/cef)

<img src="data/screenshots/screenshot1.png" alrt="Screenshot" width="800" />

</div>

## Installation

```bash
flatpak remote-add --if-not-exists flathub-beta https://flathub.org/beta-repo/flathub-beta.flatpakrepo
flatpak install flathub-beta com.stremio.Stremio
```

## Development

```bash
git clone --recurse-submodules https://github.com/Stremio/stremio-linux-shell
```

### Building

#### Fedora
```bash
dnf install mpv-devel flatpak-builder gtk3-devel libappindicator-gtk3-devel mpv-mpris
```

```bash
cargo build --release
```

#### Ubuntu
```bash
apt install build-essential libssl-dev libnss3 libmpv-dev flatpak-builder libgtk-3-dev libappindicator3-dev mpv-mpris
```

```bash
cargo build --release
```

#### Flatpak
```bash
flatpak install -y \
    org.freedesktop.Sdk//24.08 \
    org.freedesktop.Platform//24.08 \
    org.freedesktop.Sdk.Extension.rust-stable//24.08 \
    org.freedesktop.Platform.ffmpeg-full//24.08 \
    org.freedesktop.Platform.VAAPI.Intel//24.08
python3 -m pip install toml aiohttp
```

```bash
# build debug flatpak (com.stremio.Stremio.Devel)
./flatpak/build-debug.sh
# build release flatpak (com.stremio.Stremio)
./flatpak/build-release.sh
```
