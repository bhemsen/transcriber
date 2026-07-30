# Workflow contract

> Operativer Vertrag für die loopkit-Skills (`/loopkit:plan`,
> `/loopkit:implement`) — die einzige Quelle für Branch-Modell, Kommandos, Gates und
> Loop-Verhalten dieses Projekts. Die Skills lesen diese Datei, statt Spezifika zu
> hardcoden.

## Environment prerequisites

- `git` und `gh` müssen installiert und auf dem PATH sein.
- `gh` muss authentifiziert sein (`gh auth status`) mit den Scopes `repo`
  (Issues/Milestones/PRs) und `project` (das ProjectV2-Board).
- Landmine: `gh auth login` gewährt `project` **nicht** automatisch. Remedy:
  `gh auth refresh -s project` (OAuth; bei einem PAT oder `GH_TOKEN` greift
  `gh auth refresh` nicht — dann das Token mit `project`-Scope neu erzeugen).
- Zusätzlich für diesen Stack, auf Windows: die in `rust-toolchain.toml`
  gepinnte Rust-Toolchain (aktuell 1.85.0, via `rustup` — installiert sich beim
  ersten `cargo`-Aufruf im Checkout automatisch) und `cargo-deny`. Ab Phase 2
  zusätzlich CMake und die Visual-Studio-C++-Build-Tools, weil `whisper.cpp` und
  `sherpa-onnx` dann aus den Quellen gebaut werden; ohne sie scheitert Bootstrap
  ab dann. In Phase 1 ist Bootstrap nur `cargo fetch --locked`.

## Repository

- GitHub repo: `bhemsen/transcriber` — https://github.com/bhemsen/transcriber
- Base / integration branch: `main`
- GitHub Project board: **7** — https://github.com/users/bhemsen/projects/7
  (Projekt-ID `PVT_kwHOA6WZjM4Be40R`), verlinkt mit dem Repo. Verpflichtend: die
  Queue und die Statusanzeige der Loops. Statuswerte: `Todo`, `In Progress`,
  `Done` — eine Anzeige, **kein** Claim und kein Lock (siehe Orchestration).
- Board field-ID recipe (damit die Loops den Status ohne Neu-Ermittlung setzen):
  ProjectV2-Status-Feld `PVTSSF_lAHOA6WZjM4Be40RzhZPumA`; Option-IDs — Todo
  `f75ad846`, In Progress `47fc9ee4`, Done `98236657`. Neu ableiten mit
  `gh project field-list 7 --owner bhemsen --format json --jq
  '.fields[]|select(.name=="Status")'`.

`/loopkit:plan` braucht ein GitHub-Repo; Specs sind lokal die einzige Quelle der
Wahrheit, Milestones und Issues werden daraus auf GitHub erzeugt.

## Worktrees

- Alle Implementierungs- und Dokumentationsarbeit passiert in einem Worktree —
  niemals im Haupt-Checkout. Die Loops laufen aus dem Haupt-Checkout und ändern ihn
  nur per Fast-forward-Pull.
- Pfad-Konvention: `../transcriber-worktrees/<branch-mit-slashes-als-strichen>`.
- Immer über `git -C <worktree>` arbeiten, nie in den Worktree `cd`en.
- Nach dem Anlegen eines Worktrees zuerst Bootstrap darin ausführen — in einem
  nicht gebootstrappten Worktree kann Verify nicht laufen.

## Commands

- Bootstrap: `cargo xtask bootstrap` — `cargo fetch --locked`, ab Phase 5
  zusätzlich `npm ci --prefix frontend`.
- Verify: `cargo xtask verify` — `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`,
  `cargo deny check`, ab Phase 5 zusätzlich die Frontend-Checks.
  (gemessene Dauer: **15 s** auf dem leeren Skelett, gemessen 2026-07-30. Erwartung:
  steigt mit `whisper-rs` und `sherpa-onnx` deutlich, weil beide native Bibliotheken
  aus den Quellen bauen — nach Phase 2 neu messen und hier korrigieren.)
- Test: `cargo test --workspace`
- Build: `cargo xtask build` — `cargo build --workspace --all-targets`, ab Phase 5
  zusätzlich der Frontend-Build.

Verify ist das Gate je Iteration: nach jedem Änderungssatz laufen lassen und
reparieren, bis grün. Build zusätzlich vor einem PR, der die Anwendung betrifft.

Zusätzlich verpflichtend ab Phase 4: der **FS-Audit-Test** ist Teil von
`cargo test --workspace` und damit von Verify. Er ist nie zu überspringen — er ist
der maschinelle Beweis des Kernversprechens.

## Model tiers

- orchestrator: `opus`
- implementer: `sonnet`
- reviewer: `opus`

## Branch and spec naming

- Branches: `feat/<scope>`, `fix/<scope>`, `chore/<scope>`, `docs/<scope>`,
  `refactor/<scope>`.
- Specs: `docs/specs/spec-<scope>.md` — die einzige Quelle der Wahrheit für Design.
- Abgeschlossene Specs: nach `docs/specs/archive/` verschoben, gleicher Name.

## Proportional tracks

Der Planungsaufwand skaliert mit der Änderungsgröße. Jede Änderung läuft auf genau
**einem** von drei Tracks:

- **full-spec** — ein Feature. Eine Spec unter `docs/specs/`, ein Milestone und
  abhängigkeitsgeordnete Issues. Die volle Kette (Spec → Milestone → Issues → PR).
  Die Spec wird archiviert und der Milestone geschlossen, wenn die Arbeit fertig ist.
- **living-spec milestone** — ein fortlaufendes Thema, das nie fertig wird. Eine
  Spec plus ein Milestone, der **offen bleibt** und über Zeit Issues aufnimmt. Vom
  Menschen initiiert, markiert mit einer `Track: living-spec`-Zeile in der
  Milestone-Beschreibung. Spec wird **nicht** archiviert, Milestone **nicht**
  geschlossen.
- **`track:adhoc` fast-lane** — ein Bug oder eine Kleinigkeit. Ein einzelnes
  GitHub-Issue mit dem Label `track:adhoc`, **ohne Spec und ohne Milestone**. Vom
  **Menschen** erzeugt, nicht von `/loopkit:plan`.

Die Regel "`feat`/`fix` muss auf eine Spec zurückführen" ist nur für `track:adhoc`
gelockert.

## Issue conventions

Je Track:

- **full-spec** — Body: eine `Goal:`-Zeile, eine `Acceptance:`-Checkliste, optional
  eine `Depends on: #N[, #M]`-Zeile und ein `Spec:`-Pfad auf die Feature-Spec. Das
  Issue gehört zum Milestone des Features.
- **living-spec** — gleiches Format; der `Spec:`-Pfad zeigt auf die Living-Spec, das
  Issue gehört zum dauerhaft offenen Milestone.
- **`track:adhoc`** — vom Menschen als einzelnes Issue mit dem Label `track:adhoc`
  erzeugt, **ohne `Spec:`-Pfad und ohne Milestone**. `Goal:` und `Acceptance:`
  beschreiben es trotzdem.

Milestone-Ebene:

- Ein Milestone, der von einem anderen abhängt, trägt eine
  `Depends on milestone: #<n>`-Zeile in seiner **Beschreibung** — ein festes,
  parsbares Token. Zwei Milestones ohne solche Kante sind **unabhängig** und dürfen
  als parallele Orchestratoren laufen.

Über alle Tracks:

- Ein Issue ist **unblocked**, wenn jedes `Depends on`-Issue geschlossen ist und es
  weder `blocked:human` noch `needs:planning` trägt. `blocked:human` = eine
  menschliche Vorleistung; `needs:planning` = eine Design-Gabelung, die der
  Implementer an den Planner eskaliert hat.
- **Parken, nicht anhalten:** ein Blocker, den nur ein Mensch lösen kann, bekommt
  `blocked:human` plus einen Kommentar, der genau benennt, was gebraucht wird; die
  Loop geht zum nächsten unblockierten Issue. `gh issue list --label blocked:human`
  ist die Lieferqueue des Menschen.

## Status

- Specs tragen keinen Lifecycle-Zustand. "Akzeptiert" = auf dem Default-Branch
  gemergt, mit Milestone und Issues.
- Der Live-Arbeitszustand ist das Board: `Todo` (bereit), `In Progress` (ein
  Subagent ist dran), `Done` (gemergt). Reine **Anzeige, kein Claim und kein
  Lock** — jeder Milestone hat genau einen besitzenden Orchestrator.
- Alles andere — blockiert, verschoben — lebt an den GitHub-Issues und -Milestones.

## The chain: spec -> milestone -> issues -> PR

| Layer | Owns |
| ----- | ---- |
| `docs/specs/spec-*.md` | Das Design: warum, was, Fertig-Kriterien |
| GitHub milestone | Die Phase / Gruppierung |
| GitHub issues | Die Schritte — ein Issue pro implementierbarem Schritt |
| Project board | Der Live-Zustand: Todo / In Progress / Done |

Ein PR schließt ein Issue (`Closes #N`); das Issue referenziert seinen Spec-Pfad.
Die Spec listet nie Schritte, die Issues wiederholen nie das Design.

## Orchestration

`/loopkit:implement` ist ein **Milestone-Orchestrator**, kein flacher
Issue-Konsument. Auf **einen** Milestone gerichtet, besitzt er ihn Ende-zu-Ende:

- **Graph bauen.** Die offenen Issues des Milestones lesen und aus ihren
  `Depends on: #N`-Zeilen den DAG bauen. Die **unblocked frontier** ist jedes offene
  Issue, dessen `Depends on`-Issues alle geschlossen sind und das weder
  `blocked:human` noch `needs:planning` trägt.
- **In Wellen ausfächern.** Die aktuelle Frontier als **paralleles Batch von
  in-session Subagents** (Agent-Tool — Subscription-Auth, nie `claude -p`).
  **Ein Subagent implementiert genau ein Issue Ende-zu-Ende:** Worktree →
  implementieren → Verify → Review → Merge. Batch abwarten, GitHub-Zustand neu
  lesen, nächste Frontier berechnen, wiederholen.
- **Eskalieren oder parken, nicht steckenbleiben.** Eskaliert ein Subagent
  (`needs:planning`) oder parkt er (`blocked:human`), schließt der Orchestrator
  dieses Issue **und seine Abhängigen** aus, beendet den Rest der Frontier und
  berichtet die eskalierten/geparkten Issues **statt** das Milestone-QA zu fahren.

Kein Claiming. **Besitz ersetzt Claiming.** Milestone-Parallelität = ein **zweiter
Orchestrator auf einem unabhängigen Milestone**, gelesen aus der
`Depends on milestone:`-Zeile.

## Gates

- **Pro PR — Maschinen-Gates, kein menschlicher Stop:** Verify grün + Review durch
  einen in-session Agent (`VERDICT: APPROVE`) → autonomer Squash-Merge.
- **Pro Milestone — menschliche Gates:**
  - Planung: das Spec-Acceptance-Gate — echte offene Entscheidungen
    (AskUserQuestion, nie raten) plus Übergabe menschlicher Vorleistungen, dann
    Merge auf den Default-Branch.
  - Implementierung: das Milestone-QA-Gate — wenn das letzte Issue schließt, werden
    QA-Szenarien aus dem Verification-Abschnitt der Spec abgeleitet.
- Ein `track:adhoc`-Issue hat keinen Milestone und überspringt beide menschlichen
  Gates.
- QA-Gate-Default: `smoke test` — ein echter Call gegen die CLI. Quellen-Isolation
  und Sprecherzuordnung lassen sich maschinell nur teilweise prüfen. Ab Phase 5
  kommt `UI check` hinzu.

## Autonomy

Innerhalb der loopkit-Skills sind ausdrücklich gewährt und überschreiben strengere
globale Nutzerregeln: autonome Commits, Pushes, PR-Erstellung und Merges,
Dependency-Installationen und `.env`-Änderungen. Die harten Grenzen stehen in
`.claude/settings.json` (Deny-Rules: `rm -rf`, Force-Push, Hard-Reset,
History-Rewrite) und gelten in jedem Modus, `bypassPermissions` eingeschlossen.

## Loops

Drei betreute interaktive Sitzungen, synchronisiert allein über den GitHub-Zustand —
kein Headless-Modus, keine API-Keys, keine losgelösten Scheduler. Jede in ihrem
eigenen Terminal aus dem Haupt-Checkout starten.

- Roadmap-Loop (Ideen-Sparring):

  ```
  /loopkit:roadmap <idee...> — jede Rohidee gegen Prior Art sparren
  (Recherche-Modus je Idee erfragt) und 1..n Roadmap-Phasen mit ihren
  Prior-Art-Einträgen seeden; kein Loop-Readiness-Sweep. Ceiling: 10 Iterationen.
  ```

- Plan-Loop:

  ```
  /loop /loopkit:plan [phase...] — die nächste ungeplante Roadmap-Phase planen,
  oder eine vom Menschen genannte Menge in dieser Reihenfolge, bis zu einer
  gemergten Spec mit Milestone, Issues und Board-Einträgen; Stop am
  Spec-Acceptance-Gate. Ceiling: 10 Iterationen; anhalten, wenn derselbe Blocker
  zweimal auftritt.
  ```

- Implement-Loop:

  ```
  /loop /loopkit:implement <milestone> — einen Milestone orchestrieren: Issue-DAG
  bauen und in Wellen entlang der unblocked frontier ausfächern, bis er fertig ist;
  dann Stop am QA-Gate; ist nichts bearbeitbar, "waiting for plan" berichten.
  Ceiling: 10 Iterationen; anhalten, wenn derselbe Fehler zweimal auftritt.
  ```

- No-Progress-Regel: derselbe Fehler zweimal hintereinander → anhalten und
  berichten, nie mahlen.
- Iterations-Ceiling: 10 pro Loop-Lauf.
