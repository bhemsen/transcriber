# Prior Art

> Descriptive, living document. Indexed BY CONCERN, not by project. Add
> entries whenever new references surface; gaps are fine.
>
> Research mode used for the initial pass: `websearch` (2026-07-30).

## Per-process audio capture on Windows (Phase 1)

### HEnquist/wasapi-rs

- Path: `examples/record_application.rs`, `examples/processes.rs`, `examples/aec.rs`
- License: MIT
- Verdict: reuse — this is the project's linchpin capability and it already exists
  as safe Rust over the Win32 API
- Date: 2026-07-30
- Notes:
  - ADOPT: `record_application` for per-process loopback, `processes` for building
    the "pick an application" source list, `aec` as the reference for acoustic echo
    cancellation between the mic and the remote stream.
  - ADOPT: the crate's design principle — safe Rust wrappers that stay close to the
    original Win32 shapes. Keeps the Windows backend auditable against MS docs.
  - AVOID: its file-writing example bodies (they persist raw samples to disk) — our
    sink is a bounded in-memory ring buffer, never a file.

### microsoft/Windows-classic-samples — ApplicationLoopback

- Path: `Samples/ApplicationLoopback` — https://learn.microsoft.com/en-us/samples/microsoft/windows-classic-samples/applicationloopbackaudio-sample/
- License: MIT (MS sample terms)
- Verdict: reference-only — the authoritative behaviour spec for
  `ActivateAudioInterfaceAsync` + `AUDIOCLIENT_ACTIVATION_PARAMS` +
  `PROCESS_LOOPBACK_MODE`
- Date: 2026-07-30
- Notes:
  - ADOPT: `PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE` — a call client spawns
    child processes (renderers, helper processes), so the process TREE is the correct
    capture unit, not a single PID.
  - ADOPT: the documented fact that process loopback is not bound to an audio
    endpoint — one client covers all endpoints, so no per-device fan-out.
  - AVOID: assuming the mix format is queryable the usual way — `GetMixFormat`
    returns `E_NOTIMPL` on a process-loopback client; the format must be supplied.
    Known landmine, documented in MS Q&A.

## Local speech-to-text engine (Phase 2)

### openai/whisper family — whisper.cpp / faster-whisper / large-v3-turbo

- Path: model weights + `whisper.cpp` inference
- License: MIT (code and OpenAI weights)
- Verdict: reuse — the default engine; strong German quality, permissive weights
- Date: 2026-07-30
- Notes:
  - ADOPT: `large-v3-turbo` as the quality/speed default, smaller models as the
    CPU-fallback ladder.
  - AVOID: shipping weights inside the installer — download on first run, so the
    package stays small (success criterion 4).

### NVIDIA Parakeet (NeMo)

- Path: Parakeet TDT ONNX exports as used by Meetily and OpenWhispr
- License: Apache-2.0 (code) / CC-BY-4.0 (weights)
- Verdict: reference-only for MVP — markedly faster, but English-first; German is
  the primary language here
- Date: 2026-07-30
- Notes:
  - ADOPT: keep the STT engine behind a trait so Parakeet can be added later without
    touching the pipeline.
  - AVOID: making it the default while German quality is unproven for our sample.

## Speaker diarization without a Python runtime (Phase 3)

### k2-fsa/sherpa-onnx

- Path: `sherpa-onnx/csrc/sherpa-onnx-offline-speaker-diarization.cc`, Rust bindings
  (`docs.rs/sherpa-onnx`), models `sherpa-onnx-pyannote-segmentation-3-0` +
  speaker-embedding models
- License: Apache-2.0
- Verdict: reuse — solves the single hardest architectural problem: local
  diarization with no Python/PyTorch in the package
- Date: 2026-07-30
- Notes:
  - ADOPT: the segmentation -> embedding -> clustering split. We run segmentation and
    embedding INCREMENTALLY during the session and the clustering ONCE at session
    end, which yields global-context quality without ever buffering the full audio.
  - ADOPT: static linking for the Rust binding — keeps the installer a single
    artifact and avoids a runtime dependency hunt on user machines.
  - AVOID: treating its diarization as streaming — it is offline/batch by design.
    Our design accepts that: speaker labels are a session-end product.
  - CHECK: model redistribution terms per model (pyannote segmentation 3.0 exports,
    3D-Speaker, NeMo, Reverb differ) before bundling any download URL.

### diart / Streaming Sortformer

- Path: https://github.com/ainnotate/StreamingSpeakerDiarization,
  https://arxiv.org/pdf/2507.18446
- License: MIT / Apache-2.0
- Verdict: reference-only — sub-second streaming labels, but requires a bundled
  Python + PyTorch runtime (~2-3 GB), which breaks success criterion 4
- Date: 2026-07-30
- Notes:
  - ADOPT: the incremental-clustering-over-a-rolling-buffer idea as the conceptual
    model for our incremental embedding stage.
  - AVOID: the runtime cost. Revisit only if a pure-ONNX streaming model ships.
  - NOTE: overlap-aware systems cut DER by 3-7 points on meeting-like audio versus
    clustering-only baselines — the largest single accuracy lever, so the
    segmentation model must be overlap-aware.

## Produktgestalt und Desktop-Stack (Phase 5)

### Zackriya-Solutions/meetily

- Path: whole repo — Tauri + Rust backend, Next.js frontend
- License: MIT
- Verdict: reference-only — the closest competitor; adopt its stack shape, reject
  its data model
- Date: 2026-07-30
- Notes:
  - ADOPT: Tauri + Rust as the desktop stack for a local-AI app (independently
    chosen by both leading projects in this space — strong signal).
  - ADOPT: the model-provisioning and GPU-backend matrix (CUDA / Vulkan / Metal /
    CoreML / CPU) as the shape of our own fallback ladder.
  - AVOID: mixing mic + FULL system audio. That pulls unrelated applications into
    the protocol; we minimise at capture by selecting one process tree.
  - AVOID: persisting recordings. Our differentiator is that no A/V artefact exists.
  - AVOID: PRO-gating diarization out of the OSS edition — our diarization is fully
    open source. This gap is a large part of the project's justification.
  - AVOID: LLM summarisation as a core concern (their centre of gravity) — out of
    MVP scope here.

## Protokollspeicher und Aufbewahrung (Phase 4)

### fastrepl/anarlog (formerly Hyprnote)

- Path: whole repo — Tauri desktop app, Axum backend, SQLite session store
- License: MIT
- Verdict: reference-only — macOS-first, markdown-centric
- Date: 2026-07-30
- Notes:
  - ADOPT: the local SQLite session/transcript store as the persistence shape for
    protocols (text only, in our case).
  - AVOID: "attachments and recordings remain local files" — local is not the same
    as absent; we keep no recordings at all.

## Auslieferung und Modellbereitstellung (Phase 6)

### Zackriya-Solutions/meetily — Installer und Modell-Matrix

- Path: Release-Artefakte und Setup-Dokumentation des Repos
- License: MIT
- Verdict: reference-only — die Auslieferungsgestalt ist übernehmbar, die
  Paketgröße nicht
- Date: 2026-07-30
- Notes:
  - ADOPT: die GPU-Backend-Matrix (CUDA / Vulkan / Metal / CoreML / CPU) als Form
    unserer Fallback-Ladder — ein Installer, zur Laufzeit gewähltes Backend.
  - ADOPT: Modelle nicht ins Installationspaket legen, sondern beim ersten Start
    laden. Das trägt direkt das Kriterium "≤ 15 Minuten bis zum ersten Protokoll".
  - AVOID: Linux nur als "build from source". Wenn Phase 9 kommt, dann mit
    Paket-Artefakt, sonst gar nicht.
  - OFFEN: kein belastbarer Vergleich zu Signierung und SmartScreen-Verhalten für
    unsignierte Windows-Installer recherchiert — Wissenslücke, die Phase 6 in ihrer
    Spec klären muss (Kosten eines Code-Signing-Zertifikats gegen die
    Installationshürde für die Zielgruppe).

## Speaker identity and naming (Phase 7)

### OpenWhispr/openwhispr

- Path: local speaker-fingerprint store (SQLite) —
  https://openwhispr.com/blog/local-speaker-diarization
- License: MIT
- Verdict: avoid — its central mechanism is exactly our declared non-goal
- Date: 2026-07-30
- Notes:
  - AVOID: persisting speaker embeddings across sessions. Voice embeddings used for
    identification are biometric data under GDPR Art. 9 — a durable local
    fingerprint database is a heavier privacy liability than the transcript it
    labels. Our embeddings live in RAM for one session and are discarded.
  - AVOID: a cloud/BYOK fallback path. Optional cloud is still a code path that can
    leak; there is none here.

### Teams active-speaker extraction — negative result

- Path: https://learn.microsoft.com/en-us/answers/questions/438372/to-get-active-speaker-in-teams-online-meeting,
  https://techcommunity.microsoft.com/discussions/teamsdeveloper/enable-accessibility-tree-on-macos-in-the-new-teams-work-or-school/4033014
- License: —
- Verdict: avoid as a load-bearing mechanism — reference-only, exploratory at best
- Date: 2026-07-30
- Notes:
  - FINDING: there is no official API exposing the active speaker, and the new Teams
    client no longer reliably exposes its accessibility tree. Teams live captions DO
    carry speaker attribution on screen, but scraping them is undocumented, breaks
    on client updates, and requires the user to enable captions.
  - ADOPT: manual, session-scoped labelling as the PRIMARY mechanism — the user
    renames "Speaker A" to "Anna" once per session. Robust, no biometrics, no
    scraping.
  - AVOID: shipping auto-mapping as a promised feature. If attempted at all, it is
    an opt-in, best-effort extra that degrades to manual labelling.

## macOS-Backend: Per-Prozess-Audio (Phase 8)

### insidegui/AudioCap + CoreAudio Process Taps

- Path: https://github.com/insidegui/AudioCap; CoreAudio process taps, macOS 14.2+
  (any-app capture with user permission from 14.4)
- License: BSD-2-Clause
- Verdict: reuse (later) — the correct macOS path for the audio-only case
- Date: 2026-07-30
- Notes:
  - ADOPT: Core Audio process taps over ScreenCaptureKit. SCK is screen-recording
    shaped: it demands the Screen Recording permission and lights the menu-bar
    recording indicator, for a use case that captures no screen at all.
  - AVOID: requesting screen-recording entitlements we do not need — it contradicts
    the project's data-minimisation claim.

## Linux-Backend: Per-Prozess-Audio (Phase 9)

### dimtpap/obs-pipewire-audio-capture

- Path: https://github.com/dimtpap/obs-pipewire-audio-capture
- License: GPL-2.0
- Verdict: reference-only — LICENSE-INCOMPATIBLE with a permissive release. Read for
  understanding, copy nothing.
- Date: 2026-07-30
- Notes:
  - ADOPT (as knowledge, not code): PipeWire exposes each application's audio as a
    separate node, so "pick an app" is a node selection — the cleanest of the three
    platforms.
  - AVOID: any code lift. Our Linux backend must be written against the PipeWire API
    directly (`audio-capture.c` example in the PipeWire docs is the safe reference).

## Legal and consent framing (Phase 1)

### § 201 StGB and GDPR for meeting transcription (Germany/EU)

- Path: https://www.gesetze-im-internet.de/stgb/__201.html,
  https://www.unternehmensstrafrecht.de/ki-transkription-und-%C2%A7-201-stgb/,
  https://www.brandi.net/die-transkription-von-online-meetings
- License: —
- Verdict: reference-only — binding constraint on the product design
- Date: 2026-07-30
- Notes:
  - FINDING: recording the non-publicly spoken word without consent is a criminal
    offence (§ 201 StGB). Consent obtained BEFORE recording starts is the decisive
    justification. Producing only a transcript does not by itself remove the
    problem — the capture is the act.
  - FINDING: GDPR sets a HIGHER bar than criminal law. Implied consent through
    continued participation after a GDPR Art. 13/14 notice can suffice under
    criminal law, while the data-protection side wants informed, voluntary,
    case-specific consent.
  - ADOPT: an unskippable consent attestation before capture starts, recorded in the
    protocol header with a timestamp. The software makes no claim about consent
    itself — the user attests they obtained it, and the attestation is the
    documentation.
  - ADOPT: the transcript is personal data in its own right — it needs a retention
    and deletion story, not just an absence of audio.
  - AVOID: any feature that helps conceal the recording. That is the exact
    fact-pattern the offence describes.
