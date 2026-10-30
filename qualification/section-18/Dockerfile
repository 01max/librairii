# Existing qualified Ubuntu24.04 amd64 base, with runtime and observers only.
FROM ubuntu:24.04@sha256:a2d43bffc46670d7108bb3c0468c67c5557c4c92e83e1e34cbff1322ad1b94dc
ARG QUALIFICATION_UID=1001
ARG QUALIFICATION_GID=1001
RUN if ! getent group "$QUALIFICATION_GID" >/dev/null; then groupadd --gid "$QUALIFICATION_GID" qualification; fi \
    && if ! getent passwd "$QUALIFICATION_UID" >/dev/null; then \
        useradd --uid "$QUALIFICATION_UID" --gid "$QUALIFICATION_GID" --create-home --no-log-init qualification; fi
RUN apt-get update && apt-get install -y --no-install-recommends \
    libgcc-s1 libstdc++6 libfontconfig1 libfreetype6 fonts-dejavu-core fonts-noto-cjk \
    libx11-6 libx11-xcb1 libxcursor1 libxi6 libxrandr2 libxkbcommon0 libxkbcommon-x11-0 \
    libwayland-client0 libwayland-cursor0 libwayland-egl1 libegl1 libgl1 libvulkan1 mesa-vulkan-drivers \
    libdbus-1-3 libglib2.0-bin xdg-desktop-portal xdg-desktop-portal-gtk thunar \
    python3 python3-pyatspi python3-dbus python3-gi dbus-x11 at-spi2-core \
    xvfb openbox xdotool x11-utils x11-xkb-utils scrot sway wtype grim procps \
    && rm -rf /var/lib/apt/lists/*
COPY checks/ /checks/
COPY fixtures/ /fixtures/
RUN mkdir -p /work/target/native-release
ENTRYPOINT ["/usr/bin/dbus-run-session", "--", "/usr/bin/python3", "-I", "-B", "/checks/inside.py"]
