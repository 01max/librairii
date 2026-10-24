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
completion remain unqualified even if this baseline passes. Actual first-method
execution passes at `4ea1e4b` / run40081851994, with independent verification of
both eight-check baselines, all45 session hashes, actual AT-SPI focus/text,
same-toplevel Wayland trace, startup-excluded PCM/WAV and owned shutdown. Exact
identities and limits are in `accessibility-linux-observations.json`.
Compilation/unit tests alone do not qualify Orca speech. No toolkit selection
is recorded. VoiceOver remains excluded and Windows deferred by owner decision.

That first method observes speech generation but moves between controls fast
enough to interrupt individual labels. Unprompted offline tiny.en ASR produces
`(Music)` for X11 and an unreliable partial phrase for Wayland; this supplies no
positive intelligibility proof. No human listening is claimed. The next method
requires an exact actual Orca utterance and nonzero audio followed by a half-second
quiet tail for each frame/Search/Settings/Close/English/restored-Settings stage
before navigating onward. Raw byte offsets and per-stage audio facts are retained.
The existing deadlines and required scopes remain unchanged. This stronger method
is pending actual execution and does not retroactively accept earlier recordings.

The stronger method's first execution at `6da2838` / run40093774916 fails both
Search-announcement waits. Each session records its completed frame utterance
and audio, then the native Tab genuinely focuses the named Search entry. Neither
records the required Search utterance before the unchanged deadline. X11 logs
Orca's key-echo focus update before the actual focus event; Wayland reaches speech
generation but emits no Search speech record. This does not yet distinguish
event ordering from an accessibility-query stall. Artifact12559919058 ZIP SHA256
is `b2a7750a509674735de19fefbed0442a67731697e7e3af61ac510a0530142f73`.
The next diagnostic method registers a stack handler only in the private owned
Orca instance and requests a bounded failure-only stack when an utterance wait
fails. It performs no accessibility action, changes no focus and cannot qualify
missing speech. No failed result is retried or upgraded.

The diagnostic run at `30614b8` / run40095296249 captures both owned reader
stacks during the failed Search wait. Both main threads are in Orca's event-loop
entry point, with no speech-generation/text-query Python frames. Artifact12560638054
ZIP SHA256 is `9509effaebbaea058483c867cb1e20dc2a63cd131a4f9f097f728d7db7a64632`.
The inspected [Orca46.1 debug writer](https://github.com/GNOME/orca/blob/ORCA_46_1/src/orca/debug.py)
uses buffered `writelines` without a per-record flush. A local open-file
reproduction retains an entire completed speech line invisibly until flushing;
line buffering makes the next complete line immediately visible. The private
reader configuration now uses standard text-stream line buffering/write-through,
records and asserts the actual modes, and keeps all utterance/audio/deadline
requirements. This corrects transcript visibility; it does not force an
announcement or resolve the separate X11 key-echo ordering question. Fresh
native execution must establish the effect.

At `8a9d571` / run40096186659 the line-buffered Wayland session completes all
six exact utterance/audio stages and all ten baseline checks. X11 still fails
Search: its flushed transcript contains only the Tab key echo after the frame,
and records Orca's echo path setting Search as its locus with notification
disabled before the native focus event. Artifact12561087872 ZIP SHA256 is
`0d4eba32ef5b5c37df952666980bdd25f3e7d9f94e01b75824d12b55d6c173fd`.
The next v3 configured baseline disables only the supported action-key-echo
preference in the private X11 reader; all native keys, focus events, exact labels,
audio completion and deadlines remain required. Effective preferences are
recorded from the reader's first event-loop idle callback and checked by both
driver and host; Wayland retains action-key echo. The default-echo X11 route
remains a retained failure/unqualified, not a passing result. This configured
baseline cannot complete the broader2.4 gate. References:
[Orca46.1 action-key preference](https://github.com/GNOME/orca/blob/ORCA_46_1/src/orca/settings.py),
[key-event presentation and focus update](https://github.com/GNOME/orca/blob/ORCA_46_1/src/orca/scripts/default.py).

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
