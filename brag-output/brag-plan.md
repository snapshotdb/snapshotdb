# Brag Plan: anybranch

## What is this app?
anybranch gives you instant, copy-on-write branches of any production database (Postgres, MySQL, MongoDB, SQLite) on infrastructure you own — like a git branch, but for your data.

## The angle
Most "database branching" claims are marketing copy. This video shows the real thing: an actual terminal session cloning a live 100MB+ Postgres database and forking a copy-on-write branch off it in under 5 seconds — a real command, a real URL, a real number. The claim earns itself on screen instead of being asserted.

## Hook (first 2-3 seconds)
Full-bleed near-black frame. Monospace cursor blinks, then types: `$ anybranch clone prod` — cut before it resolves. The product name and the terminal are the first thing seen, not a logo card.

## Key moments (the middle)
- The `anybranch clone prod '<url>'` command resolving with a live checklist (connection ✓, wal level ✓, replica identity ✓) — the preflight checks passing in sequence.
- A single stat holds the frame: **111.7 MB replicated** — real number, not rounded marketing copy.
- The branch command — `anybranch create fix-orders --from prod --print-url` — resolving into a fresh `postgresql://` URL, with a stopwatch-style counter landing on **4.4s**.
- One-line contrast card: size of the source vs. time to branch, showing the time does not scale with size.

## Outro / punchline
"Production, forked." — then the URL from the previous scene fading into the wordmark and `snapshotdb.io`, open-source badge.

## User flow worth showing
1. Entry: `anybranch clone prod '<connection-string>'` — preflight checks resolve, replica syncs.
2. Key action: `anybranch create fix-orders --from prod --print-url` — the copy-on-write branch command.
3. Result: a fresh, isolated `postgresql://` URL returned in ~4 seconds, independent of the 111.7 MB source size.

## Tone
- Preset: app-store
- Creative direction: developer-tool product launch — clean keynote energy, terminal-native, restrained
- Interpretation: feature-forward pacing, present tense, medium-weight type, no aggression. Motion stays clean (slides/wipes), holds are confident rather than rushed, and the one big number (4.4s) gets the most visual weight in the video.

## Format: landscape — 1920x1080
## Duration: 20s

## Visual identity (from the project)
- Background: #0b0a09
- Accent / ink: #efe9df (warm off-white, used as the "bright" accent against the near-black background)
- Text: #efe9df (primary), #8f887c (muted secondary)
- Display font: Bricolage Grotesque
- Body / mono font: Space Mono
- Strongest visual element: the site's own terminal block (`$ anybranch clone prod …` / `$ anybranch create fix-orders --from prod --print-url`) and its dark, hairline-bordered "frame" aesthetic with corner tick marks

## Share copy (draft)
We cloned a 111.7MB production Postgres database and forked a copy-on-write branch off it in 4.4 seconds — real data, zero blast radius, open source. anybranch.

## Audio direction
- Role: warm, confident corporate bed with sparse, motion-matched accents
- Music: `happy-beats-business-moves-vol-1-by-ende-dot-app.mp3` (120.19 BPM) — clean, upbeat, professional
- Music treatment: starts under the hook at low-moderate volume, holds steady through the flow scenes, a small lift going into the 4.4s reveal, gentle fade-out under the outro logo card
- Music cue guidance: preset cues available for this track; target reveals near strong-beat cues in the 16-24s range (e.g. ~17.0s, ~18.5s, ~20.0s) for the branch-command resolve and the 4.4s counter landing; sequential preflight-check ticks should snap to the beat grid but each check line still needs its ~0.8s settled read time — use every-other-beat spacing, not every beat
- Audio-reactive treatment: subtle — a faint glow/presence pulse on the terminal frame tied to music energy, nothing louder than that
- SFX posture: sparse, professional — a soft key-tick while the hook command types, a light "check" tick per passing preflight line, one clean confirmation tone when the branch URL resolves
- Audio-coupled moments: the hook command typing out character by character; the preflight checklist ticking off one line at a time; the 4.4s counter landing on its final value
- Restraint rule: no whooshes, no cartoon stingers, nothing that undercuts a developer-tool's credibility

## Storyboard

### Scene 1 — Hook — 3s
Full-bleed #0b0a09 frame with the site's hairline border + corner ticks. A monospace cursor blinks, then types `$ anybranch clone prod` character by character. Cuts before the command resolves.
Sequential/interaction: yes — the command types out character by character, cursor blinking first
Audio intent: quiet anticipation, sets the terminal-native mood
Audio-coupled idea: soft key-tick per character as it types
Music: bed fades in under the typing, low volume
Transition mood: hard cut → Scene 2

### Scene 2 — Preflight resolves — 4s
The command from Scene 1 completes; a checklist appears below it and ticks off one line at a time: connection ✓, wal level ✓, replica identity ✓ (3 of the real preflight check names from the product). Each line settles fully before the next appears.
Sequential/interaction: yes — 3 checklist lines arrive one by one, each holding ~0.8s before the next
Audio intent: quiet confidence building
Audio-coupled idea: one light tick per checklist line landing
Music: steady bed, no lift yet
Transition mood: clean wipe → Scene 3

### Scene 3 — The number — 4s
Full-bleed type moment: **111.7 MB** replicated, in large display type (Bricolage Grotesque), with "replicated from production" as a small caption beneath. This is the "reveal" beat — a real number, not a rounded claim.
Sequential/interaction: none
Audio intent: a small lift, letting the number land with weight
Audio-coupled idea: number could count up quickly (0 → 111.7 MB) over ~0.6s before settling, aligned to a strong-beat cue
Music: brief lift near a strong-beat cue (~17-18s in the track if this scene lands there in the final cut; otherwise nearest strong beat)
Transition mood: slide → Scene 4

### Scene 4 — The branch — 5s
Terminal reappears: `$ anybranch create fix-orders --from prod --print-url` types out, resolves into a fresh `postgresql://you:••••@…/app` URL. A stopwatch-style counter runs alongside and lands on **4.4s**, held large enough to read clearly next to the URL.
Sequential/interaction: yes — command types, then URL resolves, then the counter lands
Audio intent: payoff — this is the claim proving itself
Audio-coupled idea: confirmation tone exactly as the counter lands on 4.4s, timed to a strong-beat cue
Music: this scene carries the clearest beat sync — land the confirmation tone on a strong cue (~20.0s or ~23.0s range)
Transition mood: smooth wipe → Scene 5

### Scene 5 — Outro / wordmark — 4s
The branch URL from Scene 4 fades into the anybranch wordmark (logo mark + "any/branch" from the site nav), with "snapshotdb.io" and an "open source · apache-2.0" label beneath, matching the site's own footer language. Final line: "Production, forked."
Sequential/interaction: none
Audio intent: settle, confident close
Audio-coupled idea: none — let the music resolve on its own
Music: gentle fade-out, no hard stop
Transition mood: soft crossfade → end

**Music mood for this video:** upbeat, clean, professional (product-launch energy, not corporate stock-photo blandness)
**Audio summary:** A steady, confident 120 BPM bed carries the whole video with three restrained sync points — the preflight checklist ticks, the 111.7 MB number landing, and the 4.4s branch confirmation — closing on a gentle fade under the wordmark.

## Voiceover script
Calm, confident, present-tense — a developer narrating their own terminal, not an ad voice. Music ducks under each line, returns between them. Scene durations should flex to match the generated audio, not the reverse.

1. (Scene 1 — hook) "This is anybranch."
2. (Scene 2 — preflight) "It connects to production and checks everything, before it touches a byte."
3. (Scene 3 — the number) "This one's real — a hundred and eleven megabytes, straight from production."
4. (Scene 4 — the branch) "Now branch it. Same data, its own database — four point four seconds, no matter the size."
5. (Scene 5 — outro) "Production, forked. anybranch — open source, self-hosted, yours."
