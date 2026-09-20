# Hyperframes Composition Brief: anybranch

## Objective
Create a short launch-style brag video for anybranch — instant copy-on-write database branching.

## Output
- Composition directory: `brag-output/composition/`
- Rendered video: `brag-output/brag.mp4`
- Format: landscape — 1920x1080
- Duration: 20 seconds

## Source Material
- Project root: `/Users/alphawave2/Desktop/Mubashir/ORIM/anybranch`
- Primary files read: `site/app/page.tsx`, `site/app/globals.css`, `site/app/layout.tsx`
- Product name: anybranch
- Tagline / strongest claim: "Branch your database like code." / "An isolated, writable copy of production in seconds — any size, any engine, on infrastructure you own."
- Key UI or visual moment to recreate: the site's own terminal block from the "how it works" section, and its dark hairline-bordered frame with corner tick marks
- Copy that must appear verbatim:
  - `anybranch clone prod 'postgresql://…/app'`
  - `anybranch create fix-orders --from prod --print-url`
  - "111.7 MB" (real number from an actual test run this session, not rounded marketing copy)
  - "4.4s" (real elapsed time for the copy-on-write branch, independent of source size)
  - "Production, forked."

## Creative Direction
- Tone preset: app-store
- Creative direction: developer-tool product launch — clean keynote energy, terminal-native, restrained
- Interpretation: feature-forward, present tense, medium-weight type, no aggression. Clean slides/wipes, confident holds, the "4.4s" number gets the most visual weight.
- Angle: Prove the claim on screen with a real terminal session instead of asserting it — clone a real ~112MB Postgres database, then branch it in 4.4 seconds, both real numbers from an actual run.
- Hook: full-bleed near-black frame, monospace cursor blinks and types `$ anybranch clone prod`, cut before it resolves.
- Outro / punchline: "Production, forked." over the wordmark and snapshotdb.io.
- Avoid:
  - Generic SaaS language ("streamline your workflow")
  - Abstract filler visuals (no stock particle/gradient washes)
  - Unrelated visual redesign — stay within the site's own dark/mono terminal aesthetic

## Visual Identity
- Background: #0b0a09
- Text / ink: #efe9df
- Muted secondary text: #8f887c
- Accent: #efe9df used as the "bright" element against near-black (no separate saturated accent color in the source)
- Display font: Bricolage Grotesque (Google Fonts) — fall back to a clean grotesk/sans if unavailable in the render environment
- Body / mono font: Space Mono (Google Fonts) — fall back to a monospace system font if unavailable
- Visual references from the project: hairline-border "frame" with corner tick marks (`.tick` elements in `globals.css`), the terminal `.term` block with 3-dot title bar, the `01/02/03/04` numbered-beat labels from the "life of a branch" section

## Storyboard
Use the storyboard in `brag-output/brag-plan.md` as the creative contract.

Scene summary:
1. Hook — 3s — `$ anybranch clone prod` types out character by character, cuts before resolving
2. Preflight resolves — 4s — 3 checklist lines (connection ✓, wal level ✓, replica identity ✓) tick in one at a time
3. The number — 4s — "111.7 MB" lands in large display type, "replicated from production" caption beneath
4. The branch — 5s — `$ anybranch create fix-orders --from prod --print-url` resolves into a real-looking `postgresql://` URL, a counter lands on "4.4s"
5. Outro / wordmark — 4s — branch URL fades into the anybranch wordmark, "snapshotdb.io", "open source · apache-2.0", final line "Production, forked."

## Audio
- Audio role: warm, confident corporate bed with sparse motion-matched accents
- Audio arc: fades in under the hook, steady through the flow scenes, small lift at the 111.7 MB reveal and the 4.4s payoff, gentle fade under the outro
- Music: `happy-beats-business-moves-vol-1-by-ende-dot-app.mp3` (120.19 BPM, bundled preset available)
- Music treatment: low-moderate under the hook, steady through scenes 2-3, small lift into scene 4's payoff, fade-out under the scene 5 wordmark — no hard stop
- Music cue guidance: bundled preset at `skills/brag/assets/music/cues/happy-beats-business-moves-vol-1-by-ende-dot-app.music-cues.json`; strong cues in the 16-24s window include ~17.0s, ~18.5s, ~20.0s, ~23.0s — use up to 2-3 of these for the 111.7 MB reveal and the 4.4s counter landing; sequential preflight ticks may use the beat grid but every-other-beat spacing (not every beat) since each line needs its own ~0.8s read time
- Audio-reactive treatment: subtle — faint glow/presence pulse on the terminal frame tied to music energy; nothing louder
- Audio-coupled moments:
  - Scene 1 hook typing — soft key-tick per character
  - Scene 2 preflight checklist — one light tick per line landing
  - Scene 3 number reveal — quick count-up (0 → 111.7 MB) over ~0.6s, aligned near a strong cue
  - Scene 4 branch payoff — one clean confirmation tone exactly as the 4.4s counter lands, aligned to a strong cue
  - Scene 5 outro — no SFX, let the music resolve
- SFX selection guidance: professional restraint — soft key/tick sounds for typing and checklist items, one clean confirmation tone for the branch payoff, nothing louder or more "gamey" than that
- SFX analysis guidance: use `skills/brag/assets/sfx/sfx-analysis.md` if present; prefer lower high-frequency-risk sounds for the repeated tick moments (typing, checklist)
- Exact SFX choice: Hyperframes should choose exact filenames, timestamps, density, and volume based on the implemented animation
- Audio files: copy the chosen music (and any Hyperframes-selected SFX) into `brag-output/composition/assets/`

## Voiceover
Voice is enabled (`--voice` was explicitly passed). Narration lines are in `brag-output/brag-plan.md` under "Voiceover script". Generate via:
```
npx hyperframes tts "<script>" --voice af_heart --output brag-output/composition/assets/voiceover.wav
```
Wire on its own audio track; duck music to 0.12-0.15 under narration, restore after. Let the generated voiceover duration set scene pacing — do not hardcode scene lengths against it; adjust `data-duration` values to match after generation.

## Hyperframes Instructions
Load the composition-building Hyperframes domain skills if available in this environment (`hyperframes-core`, `hyperframes-animation`, `hyperframes-creative`, `hyperframes-keyframes`, `hyperframes-cli`); if those skills are not registered, fall back to `npx hyperframes docs compositions`, `docs data-attributes`, and `docs gsap` as the authoritative reference, plus `npx hyperframes check`/`lint`/`render` for the build/validate/render loop. `/brag` is its own workflow — do not enter a generic promo/launch-video interview. Prefer native Hyperframes conventions over anything hardcoded in this brief.

Requirements:
- Show at least one real UI, copy, or visual element from the source project (the terminal block, the dark frame + tick-mark aesthetic).
- Keep all text readable in the final render (respect the reading-time floors from `step-2-plan.md`: ~0.8s for short labels, ~0.3s/word minimum ~1.2s for sentences).
- Keep the video within 15-25 seconds (target 20s).
- Include the planned music/SFX layer and the voiceover track.
- Treat the music cue metadata as optional timing hints — ignore cues that hurt readability or pacing.
- Use 1-3 strong-cue locks maximum in this 20s video.
- Use local assets for audio; copy music into `brag-output/composition/assets/music/` before building.
- Run `hyperframes check` before render — it is the single gate.
