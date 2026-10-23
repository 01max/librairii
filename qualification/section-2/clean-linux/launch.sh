#!/usr/bin/env bash
set -euo pipefail
# Bash's default Linux unit is 1024 bytes. SIGXFSZ makes a noisy app/compositor
# fail the probe, including output redirected away from the outer Docker log.
ulimit -f 4096
test "$(uname -m)" = x86_64
test "$(id -u)" -ne 0
cd /tmp
for tool in rustc cargo go node npm python python3 cc gcc g++ clang git; do
  if command -v "$tool" > /dev/null; then
    echo "Unexpected development tool: $tool" >&2
    exit 1
  fi
done
printf '%s\n' 'development_tools=absent' > /evidence/runtime-facts.txt
printf 'architecture=%s\nuid=%s\ncwd=%s\n' "$(uname -m)" "$(id -u)" "$PWD" >> /evidence/runtime-facts.txt
sha256sum /opt/librairii/qualification-desktop > /evidence/executable.sha256
test "$(cut -d ' ' -f 1 /evidence/executable.sha256)" = "${EXPECTED_BINARY_SHA256:?}"
dpkg-query -W '-f=${Package} ${Version}\n' > /evidence/packages.txt
cat /etc/os-release > /evidence/os-release.txt
ldd /opt/librairii/qualification-desktop > /evidence/linked-libraries.txt
if grep -q 'not found' /evidence/linked-libraries.txt; then exit 1; fi

runtime="$(mktemp -d /tmp/librairii-runtime.XXXXXX)"
chmod 700 "$runtime"
mkdir "$runtime/config" "$runtime/cache"
server=''
cleanup() {
  if [[ -n "$server" ]]; then kill "$server" 2>/dev/null || true; wait "$server" 2>/dev/null || true; fi
  rm -rf -- "$runtime"
}
trap cleanup EXIT

for session in x11 wayland; do
  if [[ "$session" == x11 ]]; then
    Xvfb :91 -screen 0 1280x900x24 -nolisten tcp > /evidence/xvfb.log 2>&1 &
    server=$!
    endpoint=/tmp/.X11-unix/X91
    display=(DISPLAY=:91)
  else
    XDG_RUNTIME_DIR="$runtime" weston --backend=headless-backend.so --renderer=pixman \
      --shell=kiosk-shell.so --socket=qualification-clean --idle-time=0 --width=1280 --height=900 \
      --no-config > /evidence/weston.log 2>&1 &
    server=$!
    endpoint="$runtime/qualification-clean"
    display=(WAYLAND_DISPLAY=qualification-clean)
  fi
  ready=false
  for ((attempt=0; attempt<100; attempt++)); do
    kill -0 "$server"
    if [[ -S "$endpoint" ]]; then ready=true; break; fi
    sleep 0.1
  done
  "$ready"
  for renderer in normal cpu; do
    backend=winit-skia
    if [[ "$renderer" == cpu ]]; then backend=winit-skia-software; fi
    name="$session-$renderer"
    mkdir "/evidence/$name"
    env -i PATH=/usr/bin:/bin LC_ALL=C.UTF-8 TMPDIR=/tmp \
      XDG_RUNTIME_DIR="$runtime" XDG_CONFIG_HOME="$runtime/config" XDG_CACHE_HOME="$runtime/cache" \
      DBUS_SESSION_BUS_ADDRESS="$DBUS_SESSION_BUS_ADDRESS" "${display[@]}" \
      SLINT_BACKEND="$backend" SLINT_DEBUG_PERFORMANCE=refresh_lazy,console WAYLAND_DEBUG=client \
      timeout --kill-after=2 30 /opt/librairii/qualification-desktop --french \
      "--renderer-smoke=/evidence/$name" > "/evidence/$name.log" 2>&1
    printf '0\n' > "/evidence/$name.exit"
  done
  # A separate fresh process on each session exercises owned cancellation/join
  # and fixture retirement after the previous app has closed.
  env -i PATH=/usr/bin:/bin LC_ALL=C.UTF-8 TMPDIR=/tmp \
    XDG_RUNTIME_DIR="$runtime" XDG_CONFIG_HOME="$runtime/config" XDG_CACHE_HOME="$runtime/cache" \
    DBUS_SESSION_BUS_ADDRESS="$DBUS_SESSION_BUS_ADDRESS" "${display[@]}" \
    SLINT_BACKEND=winit-skia-software timeout --kill-after=2 30 \
    /opt/librairii/qualification-desktop --quit-smoke > "/evidence/$session-relaunch.log" 2>&1
  printf '0\n' > "/evidence/$session-relaunch.exit"
  kill "$server"
  wait "$server" || true
  server=''
done
printf '%s\n' 'clean_runtime=passed' >> /evidence/runtime-facts.txt
