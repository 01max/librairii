# Official Ubuntu 24.04 amd64 manifest, resolved 2026-10-23. No build toolchain.
FROM ubuntu:24.04@sha256:a2d43bffc46670d7108bb3c0468c67c5557c4c92e83e1e34cbff1322ad1b94dc
ARG QUALIFICATION_UID=1001
ARG QUALIFICATION_GID=1001
RUN if ! getent group "$QUALIFICATION_GID" >/dev/null; then groupadd --gid "$QUALIFICATION_GID" qualification; fi \
    && if ! getent passwd "$QUALIFICATION_UID" >/dev/null; then \
        useradd --uid "$QUALIFICATION_UID" --gid "$QUALIFICATION_GID" --create-home --no-log-init qualification; fi
RUN apt-get update && apt-get install -y --no-install-recommends \
    libstdc++6 libgcc-s1 libfontconfig1 libfreetype6 fonts-dejavu-core fonts-noto-cjk \
    libx11-6 libx11-xcb1 libxcursor1 libxi6 libxrandr2 libxkbcommon0 libxkbcommon-x11-0 \
    libwayland-client0 libwayland-cursor0 libwayland-egl1 libegl1 libgl1 libvulkan1 \
    mesa-vulkan-drivers libdbus-1-3 \
    xvfb xauth weston dbus-daemon procps \
    && rm -rf /var/lib/apt/lists/*
# These are qualification observers, not files shipped with the application.
COPY launch.sh /usr/local/bin/qualification-clean-launch
ENTRYPOINT ["/usr/bin/dbus-run-session", "--", "/bin/bash", "/usr/local/bin/qualification-clean-launch"]
