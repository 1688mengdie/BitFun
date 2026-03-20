# Linux Development Setup

## System Requirements

### Debian/Ubuntu

```bash
sudo apt update
sudo apt install -y \
  libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  file \
  libxdo-dev \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  pkg-config \
  libglib2.0-dev \
  libgtk-3-dev \
  patchelf
```

### Arch Linux

```bash
sudo pacman -S --needed \
  webkit2gtk-4.1 \
  base-devel \
  curl \
  wget \
  file \
  openssl \
  appmenu-gtk-module \
  libappindicator-gtk3 \
  librsvg \
  xdotool
```

### Fedora

```bash
sudo dnf check-update
sudo dnf install \
  webkit2gtk4.1-devel \
  openssl-devel \
  curl \
  wget \
  file \
  libappindicator-gtk3-devel \
  librsvg2-devel \
  libxdo-devel
sudo dnf group install "c-development"
```

## Verify Installation

After installing dependencies, verify with:

```bash
# Check pkg-config can find webkit2gtk-4.1
pkg-config --modversion webkit2gtk-4.1

# Expected output: version number (e.g., 2.44.0)
```
