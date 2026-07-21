# syntax=docker/dockerfile:1

FROM debian:trixie-slim

ARG TARGETARCH
ARG VERSION_ARG="0.0"
ARG VERSION_QMP="0.0.6"
ARG VERSION_UTK="1.2.0"
ARG VERSION_VNC="1.7.0"
ARG VERSION_PASST="2026_06_11"

ARG DEBCONF_NOWARNINGS="yes"
ARG DEBIAN_FRONTEND="noninteractive"
ARG DEBCONF_NONINTERACTIVE_SEEN="true"

RUN <<EOF
  set -eu

  apt-get update
  apt-get --no-install-recommends -y install \
    bc \
    jq \
    xxd \
    tini \
    wget \
    7zip \
    curl \
    ovmf \
    fdisk \
    nginx \
    swtpm \
    procps \
    ipcalc \
    ethtool \
    iptables \
    iproute2 \
    dnsmasq \
    xz-utils \
    apt-utils \
    net-tools \
    e2fsprogs \
    qemu-utils \
    util-linux \
    websocketd \
    iputils-ping \
    genisoimage \
    inotify-tools \
    netcat-openbsd \
    ca-certificates \
    qemu-system-x86 \
    python3 \
    python3-pip \
    socat \
    imagemagick

  # Install QMP
  pip3 install --no-cache-dir --break-system-packages --root-user-action=ignore "qemu.qmp==${VERSION_QMP}"

  # Install Passt package
  wget "https://github.com/qemus/passt/releases/download/v${VERSION_PASST}/passt_${VERSION_PASST}_${TARGETARCH}.deb" -O /tmp/passt.deb -q --timeout=10
  dpkg -i /tmp/passt.deb

  apt-get clean

  # Configure QEMU
  mkdir -p /etc/qemu
  echo "allow br0" > /etc/qemu/bridge.conf

  # Install noVNC
  mkdir -p /usr/share/novnc
  wget "https://github.com/novnc/noVNC/archive/refs/tags/v${VERSION_VNC}.tar.gz" -O /tmp/novnc.tar.gz -q --timeout=10
  tar -xf /tmp/novnc.tar.gz -C /tmp/
  cd "/tmp/noVNC-${VERSION_VNC}"
  mv app core vendor package.json ./*.html /usr/share/novnc

  # Configure nginx
  unlink /etc/nginx/sites-enabled/default
  sed -i 's/^worker_processes.*/worker_processes 1;/' /etc/nginx/nginx.conf

  # Set version file
  echo "$VERSION_ARG" > /etc/version

  rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*
EOF

COPY --chmod=755 ./src /run/
COPY --chmod=755 ./web /var/www/
# F32: Hook directory for downstream customization.
# Place scripts in ./hooks/ of downstream Dockerfiles; they're
# sourced by /run/start.sh before QEMU boots. Sort order:
#   00-*.sh — early boot
#   10-*.sh — pre-config
#   20-*.sh — pre-boot (after config, before QEMU)
#   30-*.sh — post-boot (background tasks)
RUN mkdir -p /run/hooks
COPY --chmod=755 ./hooks /run/hooks/
COPY --chmod=664 ./web/conf/defaults.json /usr/share/novnc
COPY --chmod=664 ./web/conf/mandatory.json /usr/share/novnc
COPY --chmod=744 ./web/conf/nginx.conf /etc/nginx/default.conf

ADD --chmod=755 "https://github.com/qemus/fiano/releases/download/v${VERSION_UTK}/utk_${VERSION_UTK}_${TARGETARCH}.bin" /run/utk.bin

VOLUME /storage
EXPOSE 22 5900 8006

ENV BOOT="alpine"
ENV CPU_CORES="2"
ENV RAM_SIZE="2G"
ENV DISK_SIZE="64G"
ENV CLIPBOARD="N"

# ── Customization points (F32) ─────────────────────────────────────────────
#
# Downstream Dockerfiles (macos/, windows/, chromeos/) build FROM this image
# via COPY --from=ewestudios/qemu:7.37 / /. They can:
#
# 1. Add packages: add to the apt-get install list above.
#
# 2. Add hooks: COPY scripts into /run/hooks/ — they're sourced by
#    /run/start.sh before QEMU boots. Sort order controls timing.
#
# 3. Override scripts: COPY replacements into /run/ to patch individual
#    stages (./src/ → /run/). The entry.sh flow is:
#      start.sh → utils.sh → reset.sh → server.sh → install.sh →
#      disk.sh → display.sh → audio.sh → network.sh → samba.sh →
#      boot.sh → proc.sh → power.sh → memory.sh → balloon.sh →
#      config.sh → finish.sh
#
# 4. Set defaults: ENV vars override QEMU behavior. docker-compose users
#    can set these in `environment:` without rebuilding.
#
#    ┌──────────────────────┬─────────┬────────────────────────────────┐
#    │ Variable             │ Default │ Description                    │
#    ├──────────────────────┼─────────┼────────────────────────────────┤
#    │ ARGUMENTS            │ ""      │ Extra QEMU cli args appended   │
#    │ BOOT                 │ alpine  │ OS to boot/install             │
#    │ VERSION              │ latest  │ OS version                     │
#    │ RAM_SIZE             │ 2G      │ VM RAM (2G, 4G, 8G, …)       │
#    │ CPU_CORES            │ 2       │ VM vCPU count                  │
#    │ DISK_SIZE            │ 64G     │ VM disk image size             │
#    │ CLIPBOARD            │ N       │ Shared clipboard (Y/N)         │
#    │ DEBUG                │ N       │ Verbose boot logging (Y/N)     │
#    │ MONITOR              │ auto    │ HMP socket path (config.sh)    │
#    │ QMP (always on)      │ auto    │ QMP at $QEMU_DIR/qmp.sock      │
#    └──────────────────────┴─────────┴────────────────────────────────┘
#
# 5. Agentic control (F32) is built-in:
#      HMP: socat - UNIX-CONNECT:$QEMU_DIR/monitor.sock
#        → sendkey a, sendkey ctrl-alt-delete, screendump /tmp/x.ppm
#      QMP: socat - UNIX-CONNECT:$QEMU_DIR/qmp.sock
#        → {"execute":"qmp_capabilities"}
#        → {"execute":"input-send-event","arguments":{"events":[...]}}
#      Python: qemu.qmp package pre-installed for programmatic access.
#      Screenshots: imagemagick pre-installed for PPM→PNG conversion.

ENTRYPOINT ["/usr/bin/tini", "-s", "/run/entry.sh"]
