//! Measured with `scripts/benchmark` on 2026-10-16, Apple M5 Pro, macOS 27.0.
//! 1400 clips (2.9 h) from seven English test sets of the Open ASR
//! Leaderboard. Written by `scripts/benchmark/score.py --rust`. Do not edit by hand.

/// Model id, word error rate in percent, and speed (audio time over processing time).
pub(crate) const MEASURED: &[(&str, f32, u32)] = &[
    ("apple-speech", 8.05, 49),
    ("canary-1b-v2", 8.51, 5),
    ("cohere-transcribe", 6.32, 15),
    ("fluid-speech-mini", 6.61, 159),
    ("nemotron-en", 7.57, 61),
    ("nemotron-multilingual", 9.37, 52),
    ("parakeet-eou-120m", 14.08, 18),
    ("parakeet-redux", 6.94, 93),
    ("parakeet-tdt-ctc-110m", 7.97, 111),
    ("parakeet-tdt-v2", 6.37, 151),
    ("parakeet-tdt-v3", 6.91, 132),
    ("parakeet-ultra", 6.54, 148),
    ("parakeet-unified-en", 6.29, 145),
    ("phonon-2", 7.33, 143),
    ("sensevoice-small", 8.88, 30),
    ("whisper-base", 11.62, 63),
    ("whisper-distil-large-v3", 9.72, 12),
    ("whisper-large-v2", 8.57, 4),
    ("whisper-large-v3", 8.41, 4),
    ("whisper-large-v3-turbo", 8.19, 12),
    ("whisper-medium", 9.15, 8),
    ("whisper-small", 9.77, 18),
    ("whisper-tiny", 14.70, 92),
];

/// The word error rate of the live preview, for a model whose preview runs another
/// decoder than its final pass.
pub(crate) const PREVIEW: &[(&str, f32)] = &[
    ("parakeet-unified-en", 7.61),
];
