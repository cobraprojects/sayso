# Model roles

Status: built on 2026-10-15 on the branch feature/model-roles, not committed. Kroko EN on Linux stays preview only, and the popover did not change. The design is "A2 · Split card" and "A2 · Model details" on the page "Models · Rethink" in the Paper file "Sayso — v0.1 Design".

This file says how the user chooses one model for the final pass and one model for the live preview, on the Models page.

## Goal

A dictation can use two models: a streaming model for the live preview and a model for the final pass. Today the user chooses only the final model, and Sayso chooses the preview model with a fixed rule. After this change the user can choose:

- One model for both roles, for example Parakeet Unified EN or Nemotron.
- Two models, for example Parakeet EOU 120M for the preview and Whisper large-v3 for the final pass.
- One model for the final pass and no live preview.

Every streaming model can also do the final pass. A small streaming model gives rough final text (Parakeet EOU has no punctuation and no capitals), so the page says so. A first install gets the same default as today.

## Terms

These terms go into `CONTEXT.md` with the code. They replace "active model" where the text means the final pass.

**Final model**
The model that does the final pass. It can be a local model or a cloud model. The config key stays `dictation.model`, so old config files keep their choice.

**Preview model**
The streaming local model that gives the live preview. It can be the final model.

**Automatic preview**
The preview choice of a new install. Sayso uses the final model when it streams. Otherwise it uses a streaming model on disk that understands the dictation language, the small fallback model first. This is the rule of today (`preview_for`).

## Decisions to confirm

1. **Config.** `dictation.model` stays the final model. A new `dictation.preview_model` holds `"auto"` (the default) or a model id. `dictation.live_preview = false` stays the off switch. So the preview has three states: automatic, a chosen model, and off. Old config files load as automatic, which is their behavior today. No migration is necessary.
2. **Language.** If the chosen preview model does not understand the dictation language (Parakeet EOU and German), the dictation has no preview. The Models page and Settings > Audio show why and offer Automatic. Sayso does not silently use another model, because the user chose this one.
3. **Final pass for every streaming model.**
   - macOS: Parakeet EOU 120M gets `final_pass: true`. The sidecar sends the whole recording through the streaming manager, as it does for Nemotron (`transcribeStreaming`). Today it throws "has no final pass".
   - Linux: Kroko EN is preview only. A final pass needs a change in `crates/sayso-engine`. If that change is large, Kroko stays preview only in the first version.
   - Windows: the two streaming models (Parakeet TDT v2 and v3) already do the final pass.
4. **Quality note.** A catalog entry gets an optional `final_note`, a short text about the final text of the model. Parakeet EOU: "No punctuation or capitals." The page shows it when the model is the final model, or when it offers it for the final pass. The word error rate stays where it is.
5. **Surfaces that do not change.** The popover Model menu still sets the final model only. Onboarding still offers final models, and the preview stays automatic. Settings > Audio keeps its Live preview switch, which is the same `live_preview` key.
6. **Memory.** No change. `memory::wanted` keeps the final model and the preview model loaded, and loads one model once when it has both roles.

## Models page

Current design: artboard "A2 · Split card" on the Paper page "Models · Rethink".

- The top card has two slots, **Final text** and **Live preview**. Each slot shows the model, its accuracy ("1 in 13 words wrong"), a speed figure, and a **Change** menu.
- Below the card are two lists: **Non-streaming models** first ("They write your text after you stop talking."), then **Streaming models** ("They work as you talk, so they can also show the live preview."). Each list is sorted by accuracy.
- Every row has the same columns: an accuracy meter with "1 in N wrong", a speed meter, the size, and one action. A non-streaming row shows **Use**. A streaming row shows **Use** with a menu (for final text, for live preview). A model in use shows a **Final text** or **Live preview** tag. A model that is not downloaded shows **Download**.
- A note under the lists says what the accuracy figure measures, and that punctuation does not count.
- A click on a row (outside its action button) opens **Model details**, a modal. Design: artboard "A2 · Model details".

### Model details

- **Head:** the kind ("Streaming model" or "Non-streaming model") and the vendor, the name, and two sentences about the model (a new catalog text per model).
- **Accuracy:** "1 in N words wrong", and a scale from 1 in 10 to 1 in 20. The scale marks this model, the current final model, and the current preview model. Faint ticks mark the other models with a figure. One sentence compares this model with the current choices, for example "A little more accurate than your final text model, and much more accurate than your live preview." A model with no figure shows "Not measured" and no scale.
- **Facts:** speed, live preview (the update interval, or "No"), languages, punctuation and capitals, dictionary words (used or not), and the size on disk. When Sayso measured the model on this computer, speed shows the measured time ("58 ms for 8 s of speech") in the place of the rating.
- **Source:** the WER, the test sets, the chunk size of a streaming model, and that punctuation does not count.
- **Action bar:** Delete (when downloaded, with the confirmation of today) and Model card (opens the upstream page) on the left. On the right, **Use for live preview** (streaming models only) and **Use for final text**. A model that is not downloaded shows one **Download** button. A role that the model already has shows as a tag in the place of its button.
- Escape, the close button, and a click on the dimmed page close the modal.

Earlier designs, not chosen: "Models · Roles" (role columns in every row), and "A · Setup sentence" and "B · Trade-off map" on "Models · Rethink". The text below is from "Models · Roles" and stays for its decisions about states.

In short:

- The hero card becomes "Your setup", with two slots: **Final text** and **Live preview**. Each slot shows the model, its status, and a menu to change it. The Live preview menu lists Automatic, the streaming models, and Off. When one model has both roles, the hero says so in one line, and does not repeat the model.
- The list has two tables. **Streaming models** has two role columns, Final text and Live preview. **Final text only** has one role column, Final text. Both tables keep the same column lanes, so speed, size, and the Final text column line up from one table to the next.
- In a role column, a row shows **Use**, or an **In use** tag when the model has that role. A model that is not downloaded shows one **Download** button, or the download progress, across its role columns.
- The text of a row says when a model gives rough final text, for example "rough final text, no punctuation" for Parakeet EOU.
- Cloud models stay in their own section and take the final role only.

## Accuracy data

The catalog shows the word error rate (WER) on LibriSpeech test-clean today. On that set almost every model scores 2.1 to 2.3%, so a small streaming model looks as accurate as a large model. The new accuracy figure is the average WER on the eight English test sets of the Open ASR Leaderboard (AMI, Earnings22, GigaSpeech, LibriSpeech clean and other, SPGISpeech, TED-LIUM, VoxPopuli), from the snapshot of 26 March 2026 (`scripts/data/en_shortform.csv` in `huggingface/open_asr_leaderboard`) or from the model card when it uses the same eight sets. The page shows it as "1 in N words wrong" (N = 100 / WER). Punctuation and capitals do not count in this figure.

| Model | Avg WER | 1 in N | Source |
|---|---|---|---|
| Cohere Transcribe | 5.42 | 18 | leaderboard |
| Parakeet Unified EN, final pass | 5.91 | 17 | NVIDIA card |
| Parakeet TDT v2 | 6.05 | 17 | leaderboard |
| Parakeet TDT v3 | 6.32 | 16 | leaderboard |
| Parakeet Unified EN, preview (320 ms) | 6.92 | 14 | NVIDIA card |
| Nemotron Streaming EN (1120 ms) | 6.93 | 14 | NVIDIA card |
| Canary 1B v2 | 7.15 | 14 | leaderboard |
| Whisper large-v3 | 7.44 | 13 | leaderboard |
| Parakeet TDT-CTC 110M | 7.49 | 13 | leaderboard |
| Distil-Whisper large-v3 | 7.52 | 13 | leaderboard |
| Whisper large-v3 turbo | 7.83 | 13 | leaderboard |
| Whisper large-v2 | 7.84 | 13 | leaderboard |
| Parakeet EOU 120M (160 ms) | 9.30 | 11 | NVIDIA card |

No figure on the same eight sets: Parakeet Ultra, Parakeet Redux, and Phonon-2 (vendor numbers on seven sets only), Whisper medium, small, base, and tiny (only the English-only `.en` models are on the leaderboard), Nemotron Multilingual, Parakeet JA, Apple Speech, SenseVoice, and Paraformer. These models show "Not measured" and no meter. The Sayso builds are compressed (Core ML, int8, 2-bit). Nobody published scores for the compressed builds, so the figures are for the upstream checkpoints.

The accuracy meter has five steps: under 6.0%, 6.0 to 6.6, 6.6 to 8.0, 8.0 to 9.0, and over 9.0.

## Work

Each step has its own tests. The order keeps the app usable after each step.

1. **Core** (`sayso-core`): `PreviewChoice` (`Auto`, `Model(ModelId)`), the config key, `validate()` for an unknown or non-streaming preview model, and `preview_for` with the choice. Unit tests for the three states and the language rule.
   - **Catalog:** `wer_percent` becomes the Open ASR average from "Accuracy data", with `None` for "Not measured". A streaming model that is also a final model can have a second figure for its preview (Parakeet Unified EN: 5.91 final, 6.92 preview). New fields: `about` (the two sentences for Model details, an i18n key), `card_url`, `punctuation` (bool), and `preview_interval_ms` for streaming models.
2. **App state** (`sayso-app/src/model.rs`): `preview_candidate` reads the choice. `set_preview_model` and `set_live_preview` write it. The reload after a change already exists (`model.rs:488`), and the diff gets the new key.
3. **macOS engine**: final pass for Parakeet EOU, and `final_pass: true` in the catalog. Check: `real_models` with `parakeet-eou-120m` runs the final pass on both clips.
4. **Linux engine**: look at a final pass for Kroko EN. Do it only if it is small.
5. **Models page**: the hero with two slots, the row actions, the tags, the language note. i18n keys in `models.en.yml` and the four translations.
6. **Small surfaces**: the "unavailable" text of Settings > Audio, which names Parakeet EOU today, and the preview line of the onboarding practice step.
7. **Docs**: `CONTEXT.md` terms, `crates/sayso-engine-client/NOTES.md`.

## Checks

- `cargo test` for `sayso-core` and `sayso-app`, with new tests for the preview choice and `validate()`.
- `real_models` on macOS with `parakeet-eou-120m`, `nemotron-en`, and `parakeet-unified-en`: the final pass and the stream.
- The app on macOS, with screenshots of the Models page in light and dark mode, in these setups: one model for both, two models, and no preview.
- One dictation in each setup. The overlay shows the preview of the chosen model, and the transcript comes from the final model.

## Open questions

- Should the popover get a Live preview menu? This plan says no for the first version.
- Kroko EN on Linux: final pass now, or later?
