FROM ubuntu:24.04@sha256:a2d43bffc46670d7108bb3c0468c67c5557c4c92e83e1e34cbff1322ad1b94dc
ARG QUALIFICATION_UID=1001
ARG QUALIFICATION_GID=1001
RUN if ! getent group "$QUALIFICATION_GID" >/dev/null; then groupadd --gid "$QUALIFICATION_GID" qualification; fi \
    && if ! getent passwd "$QUALIFICATION_UID" >/dev/null; then \
        useradd --uid "$QUALIFICATION_UID" --gid "$QUALIFICATION_GID" --create-home --no-log-init qualification; fi
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libfontconfig1 libfreetype6 libxkbcommon-x11-0 libxcursor1 libxi6 libxrandr2 \
    libegl1 libgl1 libvulkan1 mesa-vulkan-drivers fonts-noto-cjk \
    python3 python3-pyatspi python3-dbus python3-gi dbus-x11 at-spi2-core \
    xvfb openbox xdotool x11-utils scrot sway wtype grim procps \
    orca speech-dispatcher speech-dispatcher-espeak-ng pulseaudio pulseaudio-utils \
    && rm -rf /var/lib/apt/lists/*
COPY desktop.py speech.py drive.py test_speech.py /checks/
RUN python3 -I -B /checks/test_speech.py
ENTRYPOINT ["/usr/bin/dbus-run-session", "--", "/usr/bin/python3", "-B", "/checks/drive.py"]
