# syntax=docker/dockerfile:1
#
# Android emulator built on the QEMU base — same pattern as macOS + Windows.
# The emulator runs inside QEMU's qemu-system-x86 (via the SDK emulator binary).
# Display is captured via Xvfb + x11vnc + noVNC (budtmo/docker-android pattern).

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

  # ── X11 display stack (capture emulator to VNC) ──
  # QEMU is ALREADY present from the base image (qemu-system-x86).
  # The emulator bundles its own QEMU binary — we just need X11 + Java.
  apt-get update
  apt-get --no-install-recommends -y install \
    xvfb \
    x11vnc \
    openbox \
    xterm \
    openjdk-21-jdk-headless \
    cpu-checker \
    unzip

  apt-get clean

  # Install websockify (noVNC proxy — not in base image)
  pip3 install --no-cache-dir --break-system-packages websockify

  # Set version file
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

# ── Pre-create AVD ───────────────────────────────────────────────────────
RUN echo "no" | avdmanager create avd \
    --name "test_avd" \
    --package "system-images;android-${ANDROID_API_LEVEL};default;x86_64" \
    --device "pixel_6" \
    --force

# ── Hooks + entrypoint ────────────────────────────────────────────────────
COPY --chmod=755 ./src /run/
COPY --chmod=755 ./hooks /run/hooks/

VOLUME /storage
EXPOSE 22 5554 5555 5900 6080 8006

ENV ANDROID_VERSION="14"
ENV RAM_SIZE="4G"
ENV CPU_CORES="2"
ENV DISK_SIZE="8G"
ENV ANDROID_AVD_HOME=/root/.android/avd
ENV DISPLAY=:99

ENTRYPOINT ["/usr/bin/tini", "-s", "/run/entry.sh"]
