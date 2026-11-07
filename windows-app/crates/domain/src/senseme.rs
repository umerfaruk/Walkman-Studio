//! SensMe 12-Tone analysis + channel classification, ported from
//! `Sources/WalkmanStudio/SenseMeEngine.swift`.
//!
//! Spec task 5.4. This module holds the portable math (BPM / valence / arousal /
//! mood-score extraction from mono PCM) and the classification into the 14 official
//! Sony channels, plus the SMFM payload layout.
//!
//! This crate is pure (no I/O, no platform): audio **decoding** (AVFoundation on
//! macOS, Media Foundation / ffmpeg on Windows) lives in the services/platform
//! layer. What is ported here is the signal math that runs on already-decoded mono
//! PCM — [`extract_features`] reduces a PCM window to the four acoustic features the
//! Swift `analyzeTrackOnBoard` computed inline, [`analyze_features`] reproduces the
//! tempo / valence / arousal / 10-slot STMO mood-vector math, and [`classify`]
//! reproduces the strict 14-channel + time-of-day guardrail assignment 1:1.

/// The 14 official Sony SensMe channels (8 mood + 5 time-of-day) plus the global
/// "Shuffle All Songs" pseudo-channel. Order matches the macOS carousel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SenseMeChannel {
    // Mood channels
    Energetic,
    Relax,
    Upbeat,
    Mellow,
    Emotional,
    Lounge,
    Dance,
    Extreme,
    // Time-of-day channels
    Morning,
    Daytime,
    Evening,
    Night,
    Midnight,
    // Global
    ShuffleAllSongs,
}

impl SenseMeChannel {
    /// All channels in carousel display order.
    pub const ALL: [SenseMeChannel; 14] = [
        SenseMeChannel::Morning,
        SenseMeChannel::Daytime,
        SenseMeChannel::Evening,
        SenseMeChannel::Night,
        SenseMeChannel::Midnight,
        SenseMeChannel::Relax,
        SenseMeChannel::Upbeat,
        SenseMeChannel::Energetic,
        SenseMeChannel::Emotional,
        SenseMeChannel::Lounge,
        SenseMeChannel::Dance,
        SenseMeChannel::Extreme,
        SenseMeChannel::Mellow,
        SenseMeChannel::ShuffleAllSongs,
    ];

    /// Display name, matching the macOS channel labels and cover-art filenames.
    pub fn display_name(self) -> &'static str {
        match self {
            SenseMeChannel::Energetic => "Energetic",
            SenseMeChannel::Relax => "Relax",
            SenseMeChannel::Upbeat => "Upbeat",
            SenseMeChannel::Mellow => "Mellow",
            SenseMeChannel::Emotional => "Emotional",
            SenseMeChannel::Lounge => "Lounge",
            SenseMeChannel::Dance => "Dance",
            SenseMeChannel::Extreme => "Extreme",
            SenseMeChannel::Morning => "Morning",
            SenseMeChannel::Daytime => "Daytime",
            SenseMeChannel::Evening => "Evening",
            SenseMeChannel::Night => "Night",
            SenseMeChannel::Midnight => "Midnight",
            SenseMeChannel::ShuffleAllSongs => "Shuffle all songs",
        }
    }

    /// The string the SMFM payload / macOS engine emits for this channel. These are
    /// the exact tokens produced by `analyzeTrackOnBoard` ("Shuffle All Songs" with
    /// title-cased "All"), used when round-tripping [`SenseMeInfo::channels`].
    pub fn channel_token(self) -> &'static str {
        match self {
            SenseMeChannel::ShuffleAllSongs => "Shuffle All Songs",
            other => other.display_name(),
        }
    }

    pub fn is_time_of_day(self) -> bool {
        matches!(
            self,
            SenseMeChannel::Morning
                | SenseMeChannel::Daytime
                | SenseMeChannel::Evening
                | SenseMeChannel::Night
                | SenseMeChannel::Midnight
        )
    }
}

/// Byte size of the FLAC `APPLICATION` block `SMFM` 12-Tone payload. Injection into
/// FLAC (APPLICATION block code 2) / MP3 (ID3v2.3 GEOB / TXXX) lives in the tagger.
pub const SMFM_PAYLOAD_LEN: usize = 3270;

/// SMFM payload chunk layout (tag + byte length of the chunk body), in the order the
/// Swift `buildSMFMPayload` writes them after the 4-byte `"SMFM"` magic. Each chunk is
/// preceded by a 20-byte header (`makeHeader`): 4-byte tag, `"STAE"`, `"MMLW"`, a
/// 4-byte flag word, and a 4-byte big-endian length. The sum of the magic, headers and
/// bodies is [`SMFM_PAYLOAD_LEN`] (3270).
pub const SMFM_MAGIC: [u8; 4] = *b"SMFM";
/// Header size in bytes preceding every SMFM chunk body.
pub const SMFM_CHUNK_HEADER_LEN: usize = 20;
/// `(tag, body length)` for every SMFM chunk, in write order.
pub const SMFM_CHUNK_LAYOUT: [(&str, usize); 6] = [
    ("GBPM", 4),    // big-endian f32 BPM
    ("STBF", 36),   // beat/frame table (copied from template)
    ("STSA", 8),    // 2 × big-endian u32 highlight milliseconds
    ("STMO", 160),  // 10 moods × 4 quartiles × 4 bytes
    ("STHF", 160),  // highlight frame table (copied from template)
    ("STMM", 2778), // 12-Tone mood map (copied from template, BPM patched in)
];

/// Acoustic features extracted from a mono PCM analysis window. These mirror the four
/// running statistics the Swift `analyzeTrackOnBoard` computed from the 11,025 Hz mono
/// sample stream before deriving BPM / valence / arousal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioFeatures {
    /// Root-mean-square energy of the window (0.0..~1.0).
    pub rms_energy: f64,
    /// Peak absolute amplitude in the window (0.0..1.0).
    pub peak_amplitude: f64,
    /// Zero-crossings divided by sample count (a rough brightness / noisiness proxy).
    pub zero_crossing_rate: f64,
    /// Envelope-peak rate expressed in beats per minute (0.0 when indeterminate).
    pub pulse_rate: f64,
}

impl Default for AudioFeatures {
    fn default() -> Self {
        // Swift seeds these with neutral defaults when PCM extraction yields too few
        // samples; reproduced here so callers that fail to decode stay on the same path.
        AudioFeatures {
            rms_energy: 0.12,
            peak_amplitude: 0.50,
            zero_crossing_rate: 0.15,
            pulse_rate: 0.0,
        }
    }
}

/// 12-Tone analysis result. Ported from Swift `SenseMeInfo`.
#[derive(Debug, Clone, PartialEq)]
pub struct SenseMeInfo {
    pub is_analyzed: bool,
    pub bpm: f64,
    pub raw_bpm: f64,
    pub valence: f64,
    pub arousal: f64,
    pub mood_scores: [i32; 10],
    pub channels: Vec<String>,
    pub highlight_sec: f64,
}

impl Default for SenseMeInfo {
    fn default() -> Self {
        SenseMeInfo {
            is_analyzed: false,
            bpm: 0.0,
            raw_bpm: 0.0,
            valence: 0.0,
            arousal: 0.0,
            mood_scores: [0; 10],
            channels: Vec::new(),
            highlight_sec: 0.0,
        }
    }
}

/// Minimum samples required before the extracted statistics are trusted; below this
/// the neutral [`AudioFeatures::default`] seeds are kept. Matches the Swift
/// `totalSamples > 1000` guard.
const MIN_ANALYSIS_SAMPLES: usize = 1000;

/// Reduces a mono PCM window (normalized `f32` samples in `[-1.0, 1.0]`) to the four
/// acoustic features used by [`analyze_features`]. `sample_rate` is the PCM sample
/// rate in Hz (the Swift analyzer resampled to 11,025 Hz mono before this step).
///
/// Ported 1:1 from the per-sample accumulation loop in Swift `analyzeTrackOnBoard`:
/// RMS from the mean of squares, peak absolute amplitude, zero-crossing rate, and a
/// pulse rate from envelope peaks crossing the 0.35 threshold over the window length.
pub fn extract_features(samples: &[f32], sample_rate: f64) -> AudioFeatures {
    let mut features = AudioFeatures::default();
    if samples.is_empty() || sample_rate <= 0.0 {
        return features;
    }

    let mut sum_squares: f64 = 0.0;
    let mut peak: f64 = 0.0;
    let mut zero_crossings: u64 = 0;
    let mut envelope_peaks: u64 = 0;
    // Swift seeds lastSample = 0 (treated as >= 0) and lastEnv = 0.0.
    let mut last_non_negative = true;
    let mut last_env: f64 = 0.0;
    let total_samples = samples.len();

    for &sample in samples {
        let s = sample as f64;
        let abs_s = s.abs();
        sum_squares += s * s;
        if abs_s > peak {
            peak = abs_s;
        }
        // Zero crossing: sign flip between consecutive samples. Swift treats s >= 0 as
        // the non-negative branch (lastSample starts at 0).
        let non_negative = s >= 0.0;
        if non_negative != last_non_negative {
            zero_crossings += 1;
        }
        // Envelope peak: rising edge crossing the 0.35 amplitude threshold.
        if abs_s > 0.35 && last_env <= 0.35 {
            envelope_peaks += 1;
        }
        last_env = abs_s;
        last_non_negative = non_negative;
    }

    if total_samples > MIN_ANALYSIS_SAMPLES {
        features.rms_energy = (sum_squares / total_samples as f64).sqrt();
        features.peak_amplitude = peak;
        features.zero_crossing_rate = zero_crossings as f64 / total_samples as f64;
        let time_window = total_samples as f64 / sample_rate;
        if time_window > 1.0 {
            features.pulse_rate = (envelope_peaks as f64 / time_window) * 60.0;
        }
    }

    features
}

/// Convenience wrapper for 16-bit signed PCM: normalizes by 32768 and delegates to
/// [`extract_features`]. Mirrors the `Double(s) / 32768.0` conversion in Swift.
pub fn extract_features_i16(samples: &[i16], sample_rate: f64) -> AudioFeatures {
    let normalized: Vec<f32> = samples.iter().map(|&s| s as f32 / 32768.0).collect();
    extract_features(&normalized, sample_rate)
}

/// Derived tempo / mood coordinates and the 10-slot Sony STMO mood vector, produced by
/// [`analyze_features`]. Field semantics match the Swift locals of the same name.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoodAnalysis {
    /// Final reported BPM (rounded to 1 decimal place, clamped to the genre ranges).
    pub bpm: f64,
    /// Raw BPM before the effective-tempo fold; equals `bpm` in the on-board analyzer.
    pub raw_bpm: f64,
    /// Tempo folded into the Walkman's working range (doubled below 62, halved above 180).
    pub effective_bpm: f64,
    /// Valence coordinate in `[-1.0, 1.0]`, rounded to 2 places.
    pub valence: f64,
    /// Arousal coordinate in `[-1.0, 1.0]`, rounded to 2 places.
    pub arousal: f64,
    /// 10-slot STMO mood vector, each 0..=100. Slots:
    /// 0 Energetic, 1 Relax, 2 Upbeat, 3 Mellow, 4 Emotional, 5 Lounge, 6 Dance,
    /// 7 Extreme, 8 Acoustic, 9 Electronic.
    pub mood_scores: [i32; 10],
}

#[inline]
fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    v.max(lo).min(hi)
}

/// Rounds to `places` decimal places using half-away-from-zero, matching the Swift
/// `rounded(toPlaces:)` helper (which uses `Double.rounded()` = round-half-away).
#[inline]
fn rounded_to_places(v: f64, places: i32) -> f64 {
    let divisor = 10f64.powi(places);
    (v * divisor).round() / divisor
}

/// Folds a raw BPM into the Walkman's effective working tempo: below 62 it is doubled,
/// above 180 it is halved, otherwise unchanged. Ported from the Swift one-liner used in
/// both `parseSMFMPayload` and `analyzeTrackOnBoard`.
#[inline]
pub fn effective_bpm(bpm: f64) -> f64 {
    if bpm < 62.0 {
        bpm * 2.0
    } else if bpm > 180.0 {
        bpm * 0.5
    } else {
        bpm
    }
}

/// Reproduces the tempo / valence / arousal / 10-slot STMO math of the Swift
/// `analyzeTrackOnBoard`, operating on the extracted [`AudioFeatures`] plus an optional
/// genre string (lower/upper-case insensitive; used for the same keyword bonuses and
/// floors as the macOS analyzer).
pub fn analyze_features(features: &AudioFeatures, genre: Option<&str>) -> MoodAnalysis {
    let rms_energy = features.rms_energy;
    let peak_amplitude = features.peak_amplitude;
    let zero_crossing_rate = features.zero_crossing_rate;
    let pulse_rate = features.pulse_rate;
    let g = genre.unwrap_or("").to_lowercase();

    // 4. Determine Tempo (BPM)
    let mut bpm: f64;
    if (60.0..=180.0).contains(&pulse_rate) {
        bpm = pulse_rate;
    } else if g.contains("fast")
        || g.contains("dance")
        || g.contains("house")
        || g.contains("edm")
        || g.contains("techno")
    {
        bpm = 126.0;
    } else if g.contains("slow")
        || g.contains("ballad")
        || g.contains("ambient")
        || g.contains("chill")
    {
        bpm = 78.0;
    } else if g.contains("hip-hop") || g.contains("rap") || g.contains("r&b") {
        bpm = 92.0;
    } else if g.contains("rock") || g.contains("metal") || g.contains("punk") {
        bpm = 135.0;
    } else if g.contains("reggae") || g.contains("latin") {
        bpm = 100.0;
    } else if g.contains("jazz") || g.contains("lounge") || g.contains("soul") {
        bpm = 95.0;
    } else if g.contains("classical") || g.contains("acoustic") || g.contains("folk") {
        bpm = 88.0;
    } else {
        bpm = 90.0 + (rms_energy * 150.0) + (zero_crossing_rate * 80.0);
        bpm = clamp(bpm, 68.0, 168.0);
    }
    bpm = (bpm * 10.0).round() / 10.0;
    let raw_bpm = bpm;
    let eff_bpm = effective_bpm(bpm);

    // 5. Valence & Arousal Coordinates (-1.0 to +1.0)
    let norm_bpm = clamp((eff_bpm - 110.0) / 50.0, -1.0, 1.0);
    let norm_rms = clamp((rms_energy - 0.12) / 0.10, -1.0, 1.0);
    let norm_zcr = clamp((zero_crossing_rate - 0.16) / 0.12, -1.0, 1.0);
    let dynamic_range = clamp(peak_amplitude - rms_energy, 0.0, 1.0);

    let genre_valence_bonus = if g.contains("happy")
        || g.contains("pop")
        || g.contains("dance")
        || g.contains("funk")
        || g.contains("disco")
        || g.contains("upbeat")
    {
        0.35
    } else if g.contains("sad")
        || g.contains("emotional")
        || g.contains("ballad")
        || g.contains("dark")
        || g.contains("blues")
        || g.contains("melanchol")
    {
        -0.35
    } else if g.contains("acoustic") || g.contains("folk") || g.contains("indie") {
        0.15
    } else {
        0.0
    };

    let arousal = rounded_to_places(
        clamp(
            0.40 * norm_rms + 0.35 * norm_bpm + 0.25 * norm_zcr,
            -1.0,
            1.0,
        ),
        2,
    );
    let valence = rounded_to_places(
        clamp(
            0.40 * genre_valence_bonus + 0.35 * (1.0 - norm_zcr * 0.5) + 0.25 * (norm_bpm * 0.4),
            -1.0,
            1.0,
        ),
        2,
    );

    // 6. 10-Slot Sony STMO Mood Vector (0..9) with Refined Formulas
    let score = |v: f64| -> i32 { clamp(v, 0.0, 100.0) as i32 };
    let s0 = score(50.0 + 38.0 * arousal + 15.0 * norm_bpm); // Energetic
    let s1 = score(50.0 - 38.0 * arousal - 15.0 * norm_bpm + 10.0 * valence); // Relax
    let s2 = score(50.0 + 40.0 * valence + 15.0 * norm_bpm); // Upbeat
    let s3 = score(50.0 - 25.0 * arousal + 20.0 * dynamic_range); // Mellow
    let s4 = score(40.0 + 35.0 * dynamic_range - 25.0 * valence); // Emotional
    let s5 = score(50.0 + 25.0 * (1.0 - norm_bpm.abs()) - 15.0 * arousal.abs()); // Lounge
    let mut s6 = score(
        35.0 * (pulse_rate / 130.0).min(1.0)
            + 30.0 * (rms_energy / 0.20).min(1.0)
            + 15.0 * ((norm_rms + 1.0) / 2.0)
            + 20.0 * (1.0 - (eff_bpm - 122.0).abs() / 35.0).max(0.0),
    ); // Dance
    let mut s7 = score(30.0 + 45.0 * arousal + 25.0 * norm_zcr); // Extreme
    let mut s8 = score(
        35.0 + 25.0 * (1.0 - norm_zcr) + 25.0 * dynamic_range
            - 30.0 * (rms_energy / 0.18).min(1.0)
            - 15.0 * norm_rms.max(0.0),
    ); // Acoustic
    let mut s9 = score(
        20.0 + 30.0 * (pulse_rate / 120.0).min(1.0)
            + 30.0 * (rms_energy / 0.22).min(1.0)
            + 20.0 * (1.0 - dynamic_range),
    ); // Electronic

    if g.contains("acoustic") || g.contains("folk") || g.contains("unplugged") {
        s8 = s8.max(85);
    }
    if g.contains("electronic") || g.contains("synth") || g.contains("techno") || g.contains("edm") {
        s9 = s9.max(85);
    }
    if g.contains("metal") || g.contains("hardcore") || g.contains("punk") {
        s7 = s7.max(85);
    }
    if g.contains("dance") || g.contains("club") || g.contains("house") {
        s6 = s6.max(85);
    }

    MoodAnalysis {
        bpm,
        raw_bpm,
        effective_bpm: eff_bpm,
        valence,
        arousal,
        mood_scores: [s0, s1, s2, s3, s4, s5, s6, s7, s8, s9],
    }
}

/// Maps a [`MoodAnalysis`] to the ordered, de-duplicated set of SensMe channels,
/// reproducing the "Strict 14 Official Sony Walkman Channels & Time-of-Day
/// Guardrails" logic of Swift `analyzeTrackOnBoard` / `parseSMFMPayload` 1:1.
///
/// The time-of-day channels (Morning/Daytime/Evening/Night/Midnight) are selected
/// purely from the acoustic guards — arousal and effective-BPM bounding windows that
/// describe which part of the day a track suits — exactly as the macOS engine does;
/// there is no wall-clock input. Both the 8 mood channels and the 5 time-of-day
/// channels carry a fallback so at least one of each is always assigned, and
/// "Shuffle All Songs" is always appended as the 14th global channel.
pub fn classify(analysis: &MoodAnalysis) -> Vec<SenseMeChannel> {
    let eff_bpm = analysis.effective_bpm;
    let arousal = analysis.arousal;
    let valence = analysis.valence;
    let [s0, s1, s2, s3, s4, s5, s6, s7, s8, _s9] = analysis.mood_scores;

    let mut channels: Vec<SenseMeChannel> = Vec::new();

    // A) 8 Mood Channels
    let mut mood: Vec<SenseMeChannel> = Vec::new();
    if s0 >= 55 || (arousal >= 0.35 && valence >= 0.15 && eff_bpm >= 115.0) {
        mood.push(SenseMeChannel::Energetic);
    }
    if s1 >= 55 || (arousal <= -0.15 && valence >= -0.30 && eff_bpm <= 105.0) {
        mood.push(SenseMeChannel::Relax);
    }
    if s2 >= 55 || (valence >= 0.20 && arousal >= -0.10 && arousal <= 0.65) {
        mood.push(SenseMeChannel::Upbeat);
    }
    if s3 >= 55 || (arousal >= -0.75 && arousal <= 0.05 && valence >= -0.35 && valence <= 0.45) {
        mood.push(SenseMeChannel::Mellow);
    }
    if s4 >= 50 || (valence <= -0.10 && arousal >= -0.55 && arousal <= 0.40) {
        mood.push(SenseMeChannel::Emotional);
    }
    if s7 >= 60 || arousal >= 0.65 || eff_bpm >= 145.0 {
        mood.push(SenseMeChannel::Extreme);
    }
    if s6 >= 45 || (eff_bpm >= 105.0 && eff_bpm <= 140.0 && arousal >= 0.20) {
        mood.push(SenseMeChannel::Dance);
    }
    if s5 >= 45 || (eff_bpm >= 80.0 && eff_bpm <= 120.0 && arousal >= -0.40 && arousal <= 0.30) {
        mood.push(SenseMeChannel::Lounge);
    }

    // Fallback: if 0 mood channels qualified, assign the top STMO score from slots 0..7.
    if mood.is_empty() {
        const MOOD8: [SenseMeChannel; 8] = [
            SenseMeChannel::Energetic,
            SenseMeChannel::Relax,
            SenseMeChannel::Upbeat,
            SenseMeChannel::Mellow,
            SenseMeChannel::Emotional,
            SenseMeChannel::Lounge,
            SenseMeChannel::Dance,
            SenseMeChannel::Extreme,
        ];
        let mut best_idx = 0usize;
        let mut best_score = analysis.mood_scores[0];
        for i in 1..8 {
            if analysis.mood_scores[i] > best_score {
                best_score = analysis.mood_scores[i];
                best_idx = i;
            }
        }
        mood.push(MOOD8[best_idx]);
    }

    channels.extend(mood);

    // B) 5 Time-of-Day Channels with Bounding Guards
    let mut time: Vec<SenseMeChannel> = Vec::new();
    // Morning (05:00 - 10:00)
    if (s2 >= 55 || s8 >= 60 || s1 >= 55 || valence >= 0.15)
        && (arousal >= -0.25 && arousal <= 0.40)
        && (eff_bpm >= 80.0 && eff_bpm <= 130.0)
    {
        time.push(SenseMeChannel::Morning);
    }
    // Daytime (10:00 - 17:00)
    if (s0 >= 55 || s2 >= 55 || s6 >= 55 || valence >= 0.10)
        && (arousal >= 0.05)
        && (eff_bpm >= 95.0)
    {
        time.push(SenseMeChannel::Daytime);
    }
    // Evening (17:00 - 21:00)
    if (s3 >= 55 || s1 >= 55 || s5 >= 45)
        && (arousal >= -0.45 && arousal <= 0.20)
        && (eff_bpm <= 120.0)
    {
        time.push(SenseMeChannel::Evening);
    }
    // Night (21:00 - 00:00)
    if (s5 >= 45 || s4 >= 50 || s3 >= 55)
        && (arousal <= 0.15 && valence <= 0.25)
        && (eff_bpm <= 115.0)
    {
        time.push(SenseMeChannel::Night);
    }
    // Midnight (00:00 - 05:00)
    if (s1 >= 55 || s3 >= 55) && (arousal <= 0.0) && (eff_bpm <= 90.0) {
        time.push(SenseMeChannel::Midnight);
    }

    // Circadian Fallback
    if time.is_empty() {
        if arousal >= 0.20 || eff_bpm >= 120.0 {
            time.push(SenseMeChannel::Daytime);
        } else if arousal <= -0.20 || eff_bpm <= 85.0 {
            time.push(SenseMeChannel::Midnight);
        } else if valence >= 0.15 {
            time.push(SenseMeChannel::Morning);
        } else if arousal <= 0.05 {
            time.push(SenseMeChannel::Night);
        } else {
            time.push(SenseMeChannel::Evening);
        }
    }

    channels.extend(time);

    // C) Global Device Channel (14th channel: includes all 12-Tone analyzed tracks)
    channels.push(SenseMeChannel::ShuffleAllSongs);

    // De-duplicate while preserving first-seen order (Swift NSOrderedSet behaviour).
    let mut unique: Vec<SenseMeChannel> = Vec::with_capacity(channels.len());
    for c in channels {
        if !unique.contains(&c) {
            unique.push(c);
        }
    }
    unique
}

/// Full on-board analysis of a decoded mono PCM window: extracts features, runs the
/// 12-Tone math, classifies channels, and assembles a [`SenseMeInfo`]. `duration_sec`
/// is the track duration (for the highlight-seconds field). This is the pure core of
/// the Swift `analyzeTrackOnBoard`; SMFM injection and PCM decoding stay in the
/// services/platform layer.
pub fn analyze_pcm(
    samples: &[f32],
    sample_rate: f64,
    duration_sec: f64,
    genre: Option<&str>,
) -> SenseMeInfo {
    let features = extract_features(samples, sample_rate);
    let analysis = analyze_features(&features, genre);
    let channels = classify(&analysis)
        .into_iter()
        .map(|c| c.channel_token().to_string())
        .collect();
    let highlight_sec = rounded_to_places(clamp(duration_sec * 0.35, 15.0, 95.0), 1);
    SenseMeInfo {
        is_analyzed: true,
        bpm: analysis.bpm,
        raw_bpm: analysis.raw_bpm,
        valence: analysis.valence,
        arousal: analysis.arousal,
        mood_scores: analysis.mood_scores,
        channels,
        highlight_sec,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Catalog & payload layout -------------------------------------------------

    #[test]
    fn fourteen_channels_in_order() {
        assert_eq!(SenseMeChannel::ALL.len(), 14);
        assert_eq!(SenseMeChannel::Morning.display_name(), "Morning");
        assert_eq!(
            SenseMeChannel::ShuffleAllSongs.display_name(),
            "Shuffle all songs"
        );
        assert_eq!(
            SenseMeChannel::ShuffleAllSongs.channel_token(),
            "Shuffle All Songs"
        );
    }

    #[test]
    fn time_of_day_flag() {
        assert!(SenseMeChannel::Night.is_time_of_day());
        assert!(!SenseMeChannel::Dance.is_time_of_day());
    }

    #[test]
    fn smfm_len_is_sony_spec() {
        assert_eq!(SMFM_PAYLOAD_LEN, 3270);
    }

    #[test]
    fn smfm_chunk_layout_sums_to_payload_len() {
        // 4-byte magic + each chunk's (20-byte header + body) == 3270.
        let total: usize = SMFM_MAGIC.len()
            + SMFM_CHUNK_LAYOUT
                .iter()
                .map(|(_, body)| SMFM_CHUNK_HEADER_LEN + body)
                .sum::<usize>();
        assert_eq!(total, SMFM_PAYLOAD_LEN);
    }

    // --- Feature extraction -------------------------------------------------------

    #[test]
    fn extract_features_too_few_samples_keeps_defaults() {
        // Below the 1000-sample floor the neutral seeds are preserved.
        let samples = vec![0.5f32; 500];
        let f = extract_features(&samples, 11_025.0);
        assert_eq!(f, AudioFeatures::default());
    }

    #[test]
    fn extract_features_silence_is_quiet() {
        let samples = vec![0.0f32; 4096];
        let f = extract_features(&samples, 11_025.0);
        assert_eq!(f.rms_energy, 0.0);
        assert_eq!(f.peak_amplitude, 0.0);
        // All samples are 0.0 (>= 0), so there are no sign flips and no envelope peaks.
        assert_eq!(f.zero_crossing_rate, 0.0);
        assert_eq!(f.pulse_rate, 0.0);
    }

    #[test]
    fn extract_features_full_scale_square_wave() {
        // Alternating +1 / -1 at 11.025 kHz: RMS and peak are 1.0, and every sample
        // flips sign so the zero-crossing rate approaches 1.0.
        let n = 11_025;
        let samples: Vec<f32> = (0..n)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        let f = extract_features(&samples, 11_025.0);
        assert!((f.rms_energy - 1.0).abs() < 1e-9);
        assert!((f.peak_amplitude - 1.0).abs() < 1e-9);
        // n-1 crossings over n samples.
        assert!(f.zero_crossing_rate > 0.99);
    }

    #[test]
    fn extract_features_i16_matches_normalization() {
        let i16_samples = vec![16_384i16; 2048];
        let f = extract_features_i16(&i16_samples, 11_025.0);
        // 16384 / 32768 = 0.5 → RMS 0.5, peak 0.5.
        assert!((f.rms_energy - 0.5).abs() < 1e-4);
        assert!((f.peak_amplitude - 0.5).abs() < 1e-4);
    }

    // --- Tempo / valence / arousal math -------------------------------------------

    #[test]
    fn effective_bpm_folds_extremes() {
        assert_eq!(effective_bpm(50.0), 100.0); // doubled below 62
        assert_eq!(effective_bpm(120.0), 120.0); // unchanged in range
        assert_eq!(effective_bpm(200.0), 100.0); // halved above 180
    }

    #[test]
    fn analyze_uses_pulse_rate_when_in_range() {
        let features = AudioFeatures {
            pulse_rate: 128.0,
            ..AudioFeatures::default()
        };
        let a = analyze_features(&features, None);
        assert_eq!(a.bpm, 128.0);
        assert_eq!(a.effective_bpm, 128.0);
    }

    #[test]
    fn analyze_falls_back_to_genre_tempo() {
        let features = AudioFeatures {
            pulse_rate: 0.0, // out of 60..=180 range
            ..AudioFeatures::default()
        };
        let techno = analyze_features(&features, Some("Techno"));
        assert_eq!(techno.bpm, 126.0);
        let ballad = analyze_features(&features, Some("Slow Ballad"));
        assert_eq!(ballad.bpm, 78.0);
    }

    #[test]
    fn analyze_bpm_rounds_to_one_decimal() {
        let features = AudioFeatures {
            pulse_rate: 0.0,
            rms_energy: 0.123_456,
            zero_crossing_rate: 0.1,
            ..AudioFeatures::default()
        };
        let a = analyze_features(&features, None);
        // bpm = 90 + 0.123456*150 + 0.1*80 = 116.5184 → rounds to 116.5
        assert_eq!(a.bpm, 116.5);
    }

    #[test]
    fn analyze_mood_scores_are_clamped_0_100() {
        let features = AudioFeatures {
            rms_energy: 0.9,
            peak_amplitude: 1.0,
            zero_crossing_rate: 0.9,
            pulse_rate: 170.0,
        };
        let a = analyze_features(&features, Some("EDM"));
        for s in a.mood_scores {
            assert!((0..=100).contains(&s), "score out of range: {s}");
        }
        // EDM floor pins the Electronic slot (index 9) to at least 85.
        assert!(a.mood_scores[9] >= 85);
    }

    // --- Channel classification: thresholds & guards ------------------------------

    /// Helper to build a `MoodAnalysis` directly for threshold-boundary tests.
    fn analysis(
        eff_bpm: f64,
        arousal: f64,
        valence: f64,
        mood_scores: [i32; 10],
    ) -> MoodAnalysis {
        MoodAnalysis {
            bpm: eff_bpm,
            raw_bpm: eff_bpm,
            effective_bpm: eff_bpm,
            valence,
            arousal,
            mood_scores,
        }
    }

    fn has(channels: &[SenseMeChannel], c: SenseMeChannel) -> bool {
        channels.contains(&c)
    }

    #[test]
    fn classify_always_appends_shuffle_all_songs_last() {
        let a = analysis(120.0, 0.5, 0.5, [60, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let ch = classify(&a);
        assert_eq!(*ch.last().unwrap(), SenseMeChannel::ShuffleAllSongs);
        // Exactly one occurrence.
        assert_eq!(
            ch.iter()
                .filter(|&&c| c == SenseMeChannel::ShuffleAllSongs)
                .count(),
            1
        );
    }

    #[test]
    fn classify_energetic_score_threshold_boundary() {
        // s0 == 55 qualifies via the score branch; neutral arousal/valence/bpm so the
        // acoustic branch does not fire. s0 == 54 must not qualify Energetic.
        let qualifies = analysis(100.0, -0.5, -0.5, [55, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert!(has(&classify(&qualifies), SenseMeChannel::Energetic));

        let below = analysis(100.0, -0.5, -0.5, [54, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert!(!has(&classify(&below), SenseMeChannel::Energetic));
    }

    #[test]
    fn classify_energetic_acoustic_branch_guards() {
        // No score qualifies (all < 55/50), so Energetic depends on the acoustic guard:
        // arousal >= 0.35 && valence >= 0.15 && effectiveBpm >= 115.
        let on = analysis(115.0, 0.35, 0.15, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert!(has(&classify(&on), SenseMeChannel::Energetic));

        // Just below the bpm guard: 114.9 should not qualify Energetic.
        let off = analysis(114.9, 0.35, 0.15, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert!(!has(&classify(&off), SenseMeChannel::Energetic));
    }

    #[test]
    fn classify_extreme_fires_on_high_arousal_or_bpm() {
        // arousal >= 0.65 alone qualifies Extreme.
        let by_arousal = analysis(100.0, 0.65, 0.0, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert!(has(&classify(&by_arousal), SenseMeChannel::Extreme));
        // effectiveBpm >= 145 alone qualifies Extreme.
        let by_bpm = analysis(145.0, 0.0, 0.0, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert!(has(&classify(&by_bpm), SenseMeChannel::Extreme));
        // Neither: 144.9 bpm, arousal 0.64 → no Extreme.
        let neither = analysis(144.9, 0.64, 0.0, [0, 0, 0, 0, 0, 0, 59, 0, 0, 0]);
        assert!(!has(&classify(&neither), SenseMeChannel::Extreme));
    }

    #[test]
    fn classify_mood_fallback_picks_top_score_when_none_qualify() {
        // Choose features so none of the 8 mood guards fire and all scores < their
        // thresholds, except a clear top score in slot 4 (Emotional, threshold 50 →
        // keep it at 49 so the guard branch is the only disqualifier). Use negative
        // arousal/valence in the dead zone.
        // arousal = -0.1 keeps Dance/Daytime off; valence = 0.0; bpm = 70 (eff 70).
        let scores = [10, 10, 10, 10, 49, 10, 10, 10, 10, 10];
        let a = analysis(70.0, -0.80, -0.50, scores);
        let ch = classify(&a);
        // Exactly one mood channel (the fallback) before the time channels.
        assert!(has(&ch, SenseMeChannel::Emotional));
    }

    #[test]
    fn classify_time_of_day_midnight_guard() {
        // Midnight requires (s1>=55 || s3>=55) && arousal<=0.0 && effectiveBpm<=90.
        let on = analysis(90.0, 0.0, 0.0, [0, 55, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert!(has(&classify(&on), SenseMeChannel::Midnight));

        // Arousal just above 0.0 fails the guard.
        let off_arousal = analysis(90.0, 0.01, 0.0, [0, 55, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert!(!has(&classify(&off_arousal), SenseMeChannel::Midnight));

        // BPM above 90 fails the guard.
        let off_bpm = analysis(90.1, 0.0, 0.0, [0, 55, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert!(!has(&classify(&off_bpm), SenseMeChannel::Midnight));
    }

    #[test]
    fn classify_time_of_day_daytime_guard() {
        // Daytime requires (s0>=55||s2>=55||s6>=55||valence>=0.10) && arousal>=0.05 && effBpm>=95.
        let on = analysis(95.0, 0.05, 0.10, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert!(has(&classify(&on), SenseMeChannel::Daytime));

        // arousal just below 0.05 fails.
        let off = analysis(95.0, 0.04, 0.10, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert!(!has(&classify(&off), SenseMeChannel::Daytime));
    }

    #[test]
    fn classify_time_fallback_assigns_one_when_no_guard_fires() {
        // Pick a point in the dead zone so no explicit time guard fires, forcing the
        // circadian fallback. arousal 0.10 (0.05..0.20), valence 0.0, eff bpm 100.
        // Mood scores all low so no time-channel score branch fires either.
        let a = analysis(100.0, 0.10, 0.0, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let ch = classify(&a);
        let time_count = ch.iter().filter(|c| c.is_time_of_day()).count();
        assert_eq!(time_count, 1, "exactly one time-of-day channel via fallback");
    }

    #[test]
    fn classify_no_duplicate_channels() {
        // High everything: many guards fire; ensure the result stays de-duplicated.
        let a = analysis(150.0, 0.9, 0.9, [90, 90, 90, 90, 90, 90, 90, 90, 90, 90]);
        let ch = classify(&a);
        let mut seen = ch.clone();
        seen.dedup();
        // dedup() only removes consecutive dups; compare lengths after a full unique pass.
        let mut unique = Vec::new();
        for c in &ch {
            if !unique.contains(c) {
                unique.push(*c);
            }
        }
        assert_eq!(ch.len(), unique.len(), "classify returned duplicates");
    }

    // --- End-to-end PCM analysis --------------------------------------------------

    #[test]
    fn analyze_pcm_produces_analyzed_info_with_shuffle() {
        let samples = vec![0.3f32; 8192];
        let info = analyze_pcm(&samples, 11_025.0, 200.0, Some("Pop"));
        assert!(info.is_analyzed);
        assert!(info.bpm > 0.0);
        assert!(info.channels.contains(&"Shuffle All Songs".to_string()));
        // highlight_sec = clamp(200*0.35, 15, 95) = clamp(70, 15, 95) = 70.0
        assert_eq!(info.highlight_sec, 70.0);
    }

    #[test]
    fn analyze_pcm_highlight_clamped_to_bounds() {
        let samples = vec![0.3f32; 8192];
        // Short track → highlight floored at 15s.
        let short = analyze_pcm(&samples, 11_025.0, 10.0, None);
        assert_eq!(short.highlight_sec, 15.0);
        // Very long track → capped at 95s.
        let long = analyze_pcm(&samples, 11_025.0, 1000.0, None);
        assert_eq!(long.highlight_sec, 95.0);
    }
}
