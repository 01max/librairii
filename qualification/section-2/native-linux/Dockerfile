FROM ubuntu:24.04@sha256:a2d43bffc46670d7108bb3c0468c67c5557c4c92e83e1e34cbff1322ad1b94dc
ARG QUALIFICATION_UID=1001
ARG QUALIFICATION_GID=1001
RUN if ! getent group "$QUALIFICATION_GID" >/dev/null; then groupadd --gid "$QUALIFICATION_GID" qualification; fi \
    && if ! getent passwd "$QUALIFICATION_UID" >/dev/null; then \
        useradd --uid "$QUALIFICATION_UID" --gid "$QUALIFICATION_GID" --create-home --no-log-init qualification; fi
# This separate integration observer intentionally has Python/AT-SPI tools.
# The clean-runtime image is tested independently without those tools.
RUN apt-get update && apt-get install -y --no-install-recommends \
    libfontconfig1 libfreetype6 libxkbcommon-x11-0 libxcursor1 libxi6 libxrandr2 \
    libegl1 libgl1 libvulkan1 mesa-vulkan-drivers fonts-noto-cjk \
    python3 python3-pyatspi python3-dbus python3-gi dbus-x11 at-spi2-core \
    xdg-desktop-portal xdg-desktop-portal-gtk xdg-utils file thunar \
    xvfb openbox xdotool x11-utils scrot sway wtype grim procps \
    && rm -rf /var/lib/apt/lists/*
COPY drive.py /checks/drive.py
ENTRYPOINT ["/usr/bin/dbus-run-session", "--", "/usr/bin/python3", "-I", "-B", "/checks/drive.py"]
