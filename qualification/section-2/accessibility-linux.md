# Private Orca baseline (2.4, partial)

`check-accessibility-linux.py` verifies the matching archive and runs the native
packaged spike in separate offline X11/Openbox and Wayland/Sway containers.
Each owns its D-Bus, accessibility registry, home/configuration, PulseAudio null
sink, Speech Dispatcher and eSpeak-NG module. It requests no host audio devices,
screen reader settings or input access. Only the package and evidence directory
are mounted; application payload is read-only, capabilities are dropped and
networking is disabled. Exact installed observer versions and image ID are
recorded. All processes, container names and the temporary image are retired.

The initial route records the actual app PID's AT-SPI names, roles and states,
Tab navigation into Search, native `0042` input, keyboard Settings activation,
dialog focus and Escape return, and normal Quit/fixture removal. It performs no
AT-SPI control action and refuses key delivery if the app loses OS foreground.
Orca's own speech transcript and actual eSpeak-NG output captured from the private
audio sink are retained separately. Startup speech must actually finish, and the
application WAV/nonzero-sample check excludes that greeting. Raw PCM and per-stage
byte offsets retain the original capture. Silence or a log-only speech server
cannot pass. PCM/WAV facts do not independently prove the intelligibility of each spoken
label; the retained recording requires review. Commands, transcripts, audio and
directly written evidence are bounded. Failed sessions retain their first result
and do not prevent observing the other protocol.

This is a baseline, not the complete 2.4 gate. Virtualized selection/checked
semantics, review paging, progress/error announcements and full pointer-free
completion remain unqualified even if this baseline passes. Actual execution is
pending; compilation/unit tests do not qualify Orca speech. No toolkit selection
is recorded. VoiceOver remains excluded and Windows deferred by owner decision.

The first actual run at `5de32ce` / run40081026276 records two observer setup
failures, retained in artifact12555123864 (ZIP SHA256
`8c130cd4ddb3ecd4c92bc7d9ad88053653aff35b85b23302c475f058de570bc9`).
X11 reaches actual startup speech, then asks for the active PID before any app
window exists; initial readiness now polls that observation within the unchanged
deadline, retaining command errors. Every later key still requires exact focus.
Ubuntu Orca46.1 fails on Wayland at its unconditional `xkbcomp $DISPLAY` lookup.
The reader now receives a separate private Xvfb compatibility display. The app
receives no DISPLAY, Sway still disables Xwayland, and the host requires its
actual same-toplevel Wayland configure/ack/buffer/commit trace. This does not
qualify Orca global keyboard grabs or a reader with no X11 compatibility service.
Neither failed baseline is retroactively accepted.

The observer uses Orca's documented private preferences and debug configuration
and Speech Dispatcher's private socket/configuration options. Source references:
[Orca 46.2 settings manager](https://github.com/GNOME/orca/blob/ORCA_46_2/src/orca/settings_manager.py),
[Orca debug levels](https://github.com/GNOME/orca/blob/ORCA_46_2/src/orca/debug.py),
[Orca46.1 XKB initialization](https://github.com/GNOME/orca/blob/ORCA_46_1/src/orca/orca_modifier_manager.py),
[Speech Dispatcher options](https://manpages.ubuntu.com/manpages/noble/man1/speech-dispatcher.1.html),
[Speech Dispatcher configuration](https://github.com/brailcom/speechd/blob/0.12.0/config/speechd.conf).
