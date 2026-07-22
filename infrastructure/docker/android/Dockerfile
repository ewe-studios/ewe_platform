# syntax=docker/dockerfile:1
#
# Android emulator built on the QEMU base — same pattern as macOS + Windows.
# The emulator renders to an X11 display captured by x11vnc + noVNC.
# Full QEMU (with VNC) replaces the SDK's headless binary.

FROM scratch AS runner
COPY --from=ewestudios/qemu:7.37 / /

ARG VERSION_ARG="0.0"
ARG ANDROID_SDK_ROOT=/opt/android-sdk
ARG ANDROID_API_LEVEL=34
ARG ANDROID_BUILD_TOOLS=34.0.0
ARG ANDROID_NDK_VERSION=26.1.10909125

ARG DEBCONF_NOWARNINGS="yes"
ARG DEBIAN_FRONTEND="noninteractive"
ARG DEBCONF_NONINTERACTIVE_SEEN="true"

RUN <<EOF
  set -eu

  # ── Java + Android-specific deps ──
  # X11 stack (xvfb, x11vnc, openbox, xterm, libpulse0) comes from QEMU base.
  apt-get update
  apt-get --no-install-recommends -y install \
    openjdk-21-jdk-headless \
    cpu-checker \
    unzip

  apt-get clean

  echo "$VERSION_ARG" > /etc/version
  rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*
EOF

# ── Android SDK ──────────────────────────────────────────────────────────
ENV ANDROID_SDK_ROOT=${ANDROID_SDK_ROOT}
ENV ANDROID_HOME=${ANDROID_SDK_ROOT}
ENV PATH="${ANDROID_SDK_ROOT}/cmdline-tools/latest/bin:${ANDROID_SDK_ROOT}/platform-tools:${ANDROID_SDK_ROOT}/emulator:${PATH}"

RUN mkdir -p "${ANDROID_SDK_ROOT}/cmdline-tools" \
    && cd "${ANDROID_SDK_ROOT}/cmdline-tools" \
    && wget -q https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip \
    && unzip -q commandlinetools-linux-11076708_latest.zip \
    && rm commandlinetools-linux-11076708_latest.zip \
    && mv cmdline-tools latest \
    && yes | sdkmanager --licenses > /dev/null 2>&1 \
    && sdkmanager \
        "platform-tools" \
        "build-tools;${ANDROID_BUILD_TOOLS}" \
        "platforms;android-${ANDROID_API_LEVEL}" \
        "ndk;${ANDROID_NDK_VERSION}" \
        "emulator" \
        "system-images;android-${ANDROID_API_LEVEL};default;x86_64" \
    && rm -rf "${ANDROID_SDK_ROOT}/.temp" "${ANDROID_SDK_ROOT}/.download_cache"

# ── Pre-create device AVDs ──────────────────────────────────────────────
# QEMU deps (xvfb, x11vnc, openbox, xterm, libpulse0, websockify)
# all come from the QEMU base image. Only Android-specific deps here.
# 2 phones + 2 tablets. Select at runtime via DEVICE env var.
# Available IDs from SDK: pixel_6, Nexus 5, Nexus 5X, Nexus 6, Nexus 6P,
#   Nexus 7 2013, Nexus 7, Nexus 9, Nexus 10, pixel_c, Pixel 8, etc.
#   See `avdmanager list device` for the full catalog.
# ── Hooks + entrypoint + AVD script ─────────────────────────────────────
COPY --chmod=755 ./src /run/
COPY --chmod=755 ./hooks /run/hooks/

# Pre-create 4 AVDs: 2 phones (pixel_6, nexus_5) + 2 tablets (pixel_c, nexus_7)
# Select at runtime via DEVICE env var (e.g. DEVICE=nexus_5).
ARG ANDROID_API_LEVEL
RUN /run/create-avds.sh

VOLUME /storage
EXPOSE 22 5554 5555 5900 6080 8006

ENV ANDROID_VERSION="14"
ENV RAM_SIZE="4G"
ENV CPU_CORES="2"
ENV DISK_SIZE="8G"
ENV ANDROID_AVD_HOME=/root/.android/avd
ENV DISPLAY=:99
# Emulator QEMU needs Qt + tcmalloc + WebRTC from the SDK's lib64 dir.
ENV EMULATOR_LIB64="/opt/android-sdk/emulator/lib64:/opt/android-sdk/emulator/lib64/qt/lib"
# Default device: Pixel 6. Override in docker-compose or `docker run -e DEVICE=...`
ENV DEVICE="pixel_6"

ENTRYPOINT ["/usr/bin/tini", "-s", "/run/entry.sh"]
