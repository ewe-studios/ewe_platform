# ewe-test-linux — Linux desktop test environment for foundation_platform.
#
# Runs a real X11 desktop with VNC so the Tauri app has a display. No dockurr
# needed — this is a native Docker container, not a VM-in-a-container.
# Android emulator and Windows VM are separate images (see ../android, ../windows).
#
# Built on top of the dev-linux recipe patterns from artefact/uncloud/recipes/dev-linux:
# lean base, pre-installed system deps, mise for toolchain management.
# Language toolchains (Rust specifically) are NOT baked — each project pins its
# own toolchain via mise.toml, and `mise` installs them on first use.
FROM ubuntu:24.04

ARG DEV_USER=developer

# ── System dependencies ──────────────────────────────────────────────────────
# Tauri Linux deps: GTK3, WebKitGTK, JavaScriptCore, libsoup (WebSocket in WebView)
# X11 + VNC: lightweight desktop so the app has a display
# Build essentials: Rust crates need a C compiler + pkg-config to link
# Docker CLI: for BuildKit image builds from within the container
# SSH: test harness dispatches commands via SSH
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    # Tauri system dependencies (https://tauri.app/start/prerequisites/#linux)
    libgtk-3-dev \
    libwebkit2gtk-4.1-dev \
    libjavascriptcoregtk-4.1-dev \
    libsoup-3.0-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    # Build toolchain (C compiler + pkg-config for native crate compilation)
    build-essential \
    pkg-config \
    libssl-dev \
    ca-certificates \
    curl \
    git \
    rsync \
    # X11 + lightweight window manager + VNC
    x11vnc \
    xvfb \
    openbox \
    xterm \
    # Clipboard sync between X11 selections and VNC
    autocutsel \
    # noVNC — web-based VNC client (served via nginx on port 6080)
    novnc \
    websockify \
    nginx \
    # SSH server for test command dispatch
    openssh-server \
    # Docker CLI (socket mounted from host at /var/run/docker.sock)
    docker.io \
    # Misc utilities
    wget \
    unzip \
    jq \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /var/run/sshd

# ── mise — system-wide toolchain manager ─────────────────────────────────────
# Each project pins its Rust version (and other tools) in mise.toml.
# mise installs them on first use. We don't bake rustup here.
RUN curl -fsSL https://mise.run | MISE_INSTALL_PATH=/usr/local/bin/mise sh \
    && echo 'eval "$(/usr/local/bin/mise activate bash)"' >> /etc/bash.bashrc \
    && ln -sf /usr/local/bin/mise /usr/bin/mise

# ── VNC + noVNC setup ────────────────────────────────────────────────────────
# Start Xvfb on :99, openbox as window manager, x11vnc attached to the display.
# noVNC proxies VNC to a web browser on port 6080.
COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

# ── Developer user ───────────────────────────────────────────────────────────
RUN useradd -m -s /bin/bash "${DEV_USER}" \
    && groupadd -f docker \
    && usermod -aG docker "${DEV_USER}" \
    && mkdir -p /etc/sudoers.d \
    && echo "${DEV_USER} ALL=(ALL) NOPASSWD:ALL" > /etc/sudoers.d/${DEV_USER}

# ── SSH key injection (mount your key at runtime) ────────────────────────────
# At container start, mount the test key: -v ~/.ssh/id_ed25519.pub:/tmp/authorized_keys
# The entrypoint appends it to the dev user's authorized_keys.

EXPOSE 22 5900 6080 1420

ENV DISPLAY=:99
ENV RESOLUTION=1280x720

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
