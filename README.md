# transcriber

Local meeting minutes from one application's audio — **without ever creating a
recording**.

Pick a running application (Teams, Discord, a browser tab), confirm that you have
the participants' consent, and get a speaker-attributed transcript. No bot joins
your call, no audio or video is ever written to disk, and no voice profile survives
the session.

> **Status: pre-alpha.** The foundation documents are written and the roadmap is
> sequenced; the implementation starts at phase 1. Nothing here works yet.

## Why another one

There are good local meeting transcribers already — [Meetily][meetily],
[anarlog][anarlog], [OpenWhispr][openwhispr]. They keep the audio, or they keep a
persistent voice fingerprint per speaker, and they capture the whole system mix
rather than one application. This project takes the opposite position on all three:

- **One application, not the system mix.** Audio is taken from the selected process
  tree only, so the music you were playing and the second call in another window
  never enter the protocol. Data minimisation happens at capture — what is never
  captured needs no deletion.
- **Zero A/V persistence, by architecture.** Samples live in a fixed-size in-memory
  ring buffer. The crate that writes files has no dependency on the crate that owns
  audio buffers, so "audio cannot be written" is a property of the dependency graph
  rather than a promise in a README.
- **No voice fingerprints.** Speaker embeddings exist in RAM for one session and are
  zeroed at the end. Voice embeddings used for identification are biometric data
  under GDPR Art. 9; a durable local fingerprint database is a heavier liability
  than the transcript it labels.
- **Fully open speaker diarization.** Not gated into a paid tier.

The evidence per alternative — what is adopted, what is deliberately avoided — is
in [`docs/prior-art.md`](docs/prior-art.md).

## Legal notice — please read before using this

**This tool is not for covert recording, and it will never help you hide one.**

Recording or transcribing a non-public conversation without the participants'
consent is a criminal offence in Germany (§ 201 StGB, up to three years) and
unlawful in many other jurisdictions. Producing "only a transcript" does not change
that: the capture itself is the act. Under the GDPR the transcript is personal data
in its own right and needs its own legal basis and deletion concept.

Therefore:

- The application requires you to **confirm that you have obtained consent** before
  capture starts. There is no code path that starts capture without it, and the
  confirmation is written into the protocol header with a timestamp.
- The software makes **no claim about consent itself** and does not inform anyone on
  your behalf. Telling the other participants is your job, before you press start.
- There will be **no stealth, anti-detection, or indicator-suppression features**,
  and pull requests adding them will be rejected. See the non-goals in
  [`docs/vision.md`](docs/vision.md).

Nichts davon ist Rechtsberatung. Wer in einem betrieblichen Kontext aufzeichnet,
klärt das vorher mit Datenschutzbeauftragten und Betriebsrat.

<!-- markdownlint-disable-next-line MD026 -->
## Documentation

| Document | Content |
| --- | --- |
| [`docs/vision.md`](docs/vision.md) | What and why, success criteria, non-goals |
| [`docs/constitution.md`](docs/constitution.md) | Binding rules: stack, principles, quality gates |
| [`docs/architecture.md`](docs/architecture.md) | Components, boundaries, flows |
| [`docs/roadmap.md`](docs/roadmap.md) | The sequenced phases |
| [`docs/prior-art.md`](docs/prior-art.md) | Referenced projects, per concern |
| [`docs/workflow.md`](docs/workflow.md) | Branch model, commands, gates |

The foundation documents are written in German; code, identifiers, commits, and
issues are English.

## Development

Prerequisites on Windows: the Rust MSVC toolchain. From phase 2 on, also CMake
and the Visual Studio C++ Build Tools, once `whisper.cpp` and `sherpa-onnx` are
built from source.

```sh
cargo xtask bootstrap   # make a fresh checkout runnable
cargo xtask verify      # the gate: fmt, clippy, tests, license check
```

## License

Apache-2.0. See [`LICENSE`](LICENSE).

[meetily]: https://github.com/Zackriya-Solutions/meetily
[anarlog]: https://github.com/fastrepl/anarlog
[openwhispr]: https://github.com/OpenWhispr/openwhispr
