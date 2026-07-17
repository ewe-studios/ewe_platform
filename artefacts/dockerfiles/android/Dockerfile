# ewe-test-android — Android test environment for foundation_platform.
#
# Extends the Linux desktop image with Android SDK, emulator, and ADB.
# The emulator runs on the host's KVM for hardware acceleration.
# dockurr is NOT used here — the emulator runs natively in the container.
#
# Android SDK setup follows the patterns from Tauri's own Android CI docs
# and the dockurr pattern of baking everything into the image at build time
# rather than downloading at container start (which is slow + flaky).
FROM ubuntu:24.04

ARG DEV_USER=developer
ARG ANDROID_SDK_ROOT=/opt/android-sdk
ARG ANDROID_API_LEVEL=34
ARG ANDROID_BUILD_TOOLS=34.0.0
ARG ANDROID_NDK_VERSION=26.1.10909125

# ── System dependencies ──────────────────────────────────────────────────────
# Same Linux desktop + Tauri deps as linux/Dockerfile, plus:
#   - KVM: hardware acceleration for Android emulator
#   - Java 17: required by Android SDK / Gradle
#   - ADB: Android Debug Bridge for test command dispatch
#   - usbutils + libusb: for physical device testing (optional)
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    # Tauri system deps (same as linux/Dockerfile)
    libgtk-3-dev \
    libwebkit2gtk-4.1-dev \
    libjavascriptcoregtk-4.1-dev \
    libsoup-3.0-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    # Build toolchain
    build-essential \
    pkg-config \
    libssl-dev \
    ca-certificates \
    curl \
    git \
    rsync \
    # X11 + VNC (same as linux/Dockerfile)
    x11vnc \
    xvfb \
    openbox \
    xterm \
    novnc \
    websockify \
    nginx \
    openssh-server \
    docker.io \
    wget \
    unzip \
    jq \
    # Android-specific
    openjdk-17-jdk-headless \
    cpu-checker \
    qemu-kvm \
    libvirt-daemon-system \
    bridge-utils \
    usbutils \
    libusb-1.0-0-dev \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /var/run/sshd

# ── mise ─────────────────────────────────────────────────────────────────────
RUN curl -fsSL https://mise.run | MISE_INSTALL_PATH=/usr/local/bin/mise sh \
    && echo 'eval "$(/usr/local/bin/mise activate bash)"' >> /etc/bash.bashrc \
    && ln -sf /usr/local/bin/mise /usr/bin/mise

# ── Android SDK ──────────────────────────────────────────────────────────────
# Install commandlinetools first, then use sdkmanager to install the rest.
# This follows Tauri's mobile setup guide: https://v2.tauri.app/start/prerequisites/#android
ENV ANDROID_SDK_ROOT=${ANDROID_SDK_ROOT}
ENV ANDROID_HOME=${ANDROID_SDK_ROOT}
ENV PATH="${ANDROID_SDK_ROOT}/cmdline-tools/latest/bin:${ANDROID_SDK_ROOT}/platform-tools:${ANDROID_SDK_ROOT}/emulator:${PATH}"

RUN mkdir -p "${ANDROID_SDK_ROOT}/cmdline-tools" \
    && cd "${ANDROID_SDK_ROOT}/cmdline-tools" \
    && wget -q https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip \
    && unzip -q commandlinetools-linux-11076708_latest.zip \
    && rm commandlinetools-linux-11076708_latest.zip \
    && mv cmdline-tools latest \
    # Accept licenses upfront (non-interactive)
    && yes | sdkmanager --licenses > /dev/null 2>&1 \
    # Install platform tools, build tools, platform, NDK, emulator
    && sdkmanager \
        "platform-tools" \
        "build-tools;${ANDROID_BUILD_TOOLS}" \
        "platforms;android-${ANDROID_API_LEVEL}" \
        "ndk;${ANDROID_NDK_VERSION}" \
        "emulator" \
        "system-images;android-${ANDROID_API_LEVEL};default;x86_64" \
    # Clean up cache to keep image lean
    && sdkmanager --list | grep -v '^$' || true \
    && rm -rf "${ANDROID_SDK_ROOT}/.temp" "${ANDROID_SDK_ROOT}/.download_cache"

# ── Create AVD (Android Virtual Device) ──────────────────────────────────────
# Pre-create an emulator image. The first boot still takes time, but the image
# is already downloaded and the AVD is configured.
RUN echo "no" | avdmanager create avd \
    --name "test_avd" \
    --package "system-images;android-${ANDROID_API_LEVEL};default;x86_64" \
    --device "pixel_6" \
    --force

# ── VNC + entrypoint ─────────────────────────────────────────────────────────
COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

# ── Developer user ───────────────────────────────────────────────────────────
RUN useradd -m -s /bin/bash "${DEV_USER}" \
    && groupadd -f docker \
    && usermod -aG docker "${DEV_USER}" \
    && usermod -aG kvm "${DEV_USER}" \
    && echo "${DEV_USER} ALL=(ALL) NOPASSWD:ALL" > /etc/sudoers.d/${DEV_USER}

EXPOSE 22 5901 6081 5037 5554 5555

# Android emulator uses these ports:
#   5554 = emulator console (ADB)
#   5555 = second emulator instance (if needed)
#   5037 = ADB server

ENV DISPLAY=:99
ENV RESOLUTION=1280x720
ENV ANDROID_AVD_HOME=/root/.android/avd
ENV ANDROID_EMULATOR_HEADLESS=true

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
