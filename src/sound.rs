//! Sound effects. Every sound is synthesized once at start-up from sine waves
//! and filtered noise, so no audio files are needed.
//!
//! Playback uses `rodio` and is part of the default `sound` cargo feature. Built
//! with `--no-default-features`, or when no audio device can be opened, the game
//! runs silently and the volume setting says why.

/// Sample rate of the synthesized sounds.
pub const SAMPLE_RATE: u32 = 44_100;
const SR: f32 = SAMPLE_RATE as f32;
const TAU: f32 = std::f32::consts::TAU;

/// The sounds the game can make.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cue {
    /// Hole cards being dealt.
    Deal,
    /// Three cards for the flop.
    Flop,
    /// One card (turn or river).
    Card,
    Fold,
    Check,
    /// A call: a couple of chips.
    Chip,
    /// A bet or raise: a small stack of chips.
    Bet,
    /// All-in: a heap of chips and a thump.
    AllIn,
    /// The pot being pushed to someone else.
    Pot,
    /// You won chips.
    Win,
    /// You were eliminated.
    Lose,
    /// An opponent was eliminated.
    Bust,
    /// You won the tournament.
    Fanfare,
    /// It is your turn.
    Turn,
    /// Menu navigation.
    Tick,
}

impl Cue {
    pub const ALL: [Cue; 15] = [
        Cue::Deal,
        Cue::Flop,
        Cue::Card,
        Cue::Fold,
        Cue::Check,
        Cue::Chip,
        Cue::Bet,
        Cue::AllIn,
        Cue::Pot,
        Cue::Win,
        Cue::Lose,
        Cue::Bust,
        Cue::Fanfare,
        Cue::Turn,
        Cue::Tick,
    ];
}

// ------------------------------------------------------------------ synthesis

/// Xorshift noise source, deterministic so every run sounds the same.
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

fn samples(seconds: f32) -> usize {
    (seconds.max(0.0) * SR) as usize
}

/// Adds `src` into `out`, starting `at` seconds in.
fn mix(out: &mut Vec<f32>, at: f32, src: &[f32]) {
    let start = samples(at);
    if out.len() < start + src.len() {
        out.resize(start + src.len(), 0.0);
    }
    for (i, s) in src.iter().enumerate() {
        out[start + i] += s;
    }
}

/// Short linear fades at both ends so nothing clicks.
fn fade_edges(buf: &mut [f32], fade_seconds: f32) {
    let n = buf.len();
    if n == 0 {
        return;
    }
    let f = samples(fade_seconds).min(n / 2).max(1);
    for i in 0..f {
        let g = i as f32 / f as f32;
        buf[i] *= g;
        buf[n - 1 - i] *= g;
    }
}

/// A bell-like tone: sine plus a little second and third harmonic, fast attack,
/// exponential decay.
fn tone(freq: f32, dur: f32, amp: f32, decay: f32) -> Vec<f32> {
    let n = samples(dur);
    let attack = samples(0.004).max(1) as f32;
    let mut buf: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / SR;
            let env = (i as f32 / attack).min(1.0) * (-decay * t).exp();
            let ph = TAU * freq * t;
            amp * env * (ph.sin() + 0.25 * (2.0 * ph).sin() + 0.08 * (3.0 * ph).sin())
        })
        .collect();
    fade_edges(&mut buf, 0.004);
    buf
}

/// A low drum-like thump whose pitch drops quickly.
fn thump(freq: f32, dur: f32, amp: f32) -> Vec<f32> {
    let n = samples(dur);
    let mut phase = 0.0f32;
    let mut buf: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / SR;
            let f = freq * (1.0 + 0.8 * (-t * 30.0).exp());
            phase += TAU * f / SR;
            amp * (-t * 6.0 / dur).exp() * phase.sin()
        })
        .collect();
    fade_edges(&mut buf, 0.003);
    buf
}

/// The click of two chips touching: a few inharmonic partials that die away fast.
fn click(base: f32, dur: f32, amp: f32, decay: f32) -> Vec<f32> {
    let partials = [(1.0f32, 1.0f32), (1.47, 0.6), (2.09, 0.4), (2.83, 0.25)];
    let n = samples(dur);
    let mut buf: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / SR;
            let env = (-decay * t).exp();
            let s: f32 = partials
                .iter()
                .map(|&(mult, gain)| gain * (TAU * base * mult * t).sin())
                .sum();
            amp * env * s * 0.45
        })
        .collect();
    fade_edges(&mut buf, 0.002);
    buf
}

/// Filtered noise. `smooth` close to 1 gives a dull sound, close to 0 a bright one;
/// `highpass` keeps only what the filter removed (a crisp, papery sound).
fn noise_burst(
    noise: &mut Noise,
    dur: f32,
    amp: f32,
    smooth: f32,
    highpass: bool,
    envelope: impl Fn(f32) -> f32,
) -> Vec<f32> {
    let n = samples(dur);
    let mut low = 0.0f32;
    let mut buf: Vec<f32> = (0..n)
        .map(|i| {
            let x = noise.next();
            low = smooth * low + (1.0 - smooth) * x;
            let y = if highpass { x - low } else { low };
            amp * envelope(i as f32 / n.max(1) as f32) * y
        })
        .collect();
    fade_edges(&mut buf, 0.002);
    buf
}

/// A card sliding off the deck.
fn flick(noise: &mut Noise) -> Vec<f32> {
    noise_burst(noise, 0.045, 0.55, 0.55, true, |x| (-x * 5.0).exp())
}

/// Several chips landing on the felt.
fn chips(out: &mut Vec<f32>, noise: &mut Noise, count: usize, spacing: f32, amp: f32) {
    for k in 0..count {
        let at = (k as f32 * spacing + noise.next().abs() * spacing * 0.4).max(0.0);
        let base = 2900.0 + 700.0 * noise.next();
        mix(out, at, &click(base, 0.07, amp, 75.0));
    }
}

/// Scales the buffer down (never up) so its peak is at most `peak`.
fn normalize(buf: &mut [f32], peak: f32) {
    let max = buf.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    if max > peak {
        let g = peak / max;
        for s in buf.iter_mut() {
            *s *= g;
        }
    }
}

/// Builds the mono samples (at [`SAMPLE_RATE`]) for one cue.
pub fn synthesize(cue: Cue) -> Vec<f32> {
    let mut noise = Noise(0x9E37_79B9u32.wrapping_mul(cue as u32 + 7) | 1);
    let mut out: Vec<f32> = Vec::new();
    match cue {
        Cue::Deal => {
            for k in 0..4 {
                mix(&mut out, k as f32 * 0.075, &flick(&mut noise));
            }
        }
        Cue::Flop => {
            for k in 0..3 {
                mix(&mut out, k as f32 * 0.09, &flick(&mut noise));
            }
        }
        Cue::Card => mix(&mut out, 0.0, &flick(&mut noise)),
        Cue::Fold => {
            let swoosh = noise_burst(&mut noise, 0.22, 0.5, 0.85, false, |x| {
                (std::f32::consts::PI * x).sin().powi(2)
            });
            mix(&mut out, 0.0, &swoosh);
        }
        Cue::Check => {
            let knock = thump(190.0, 0.08, 0.7);
            mix(&mut out, 0.0, &knock);
            mix(&mut out, 0.11, &knock);
        }
        Cue::Chip => chips(&mut out, &mut noise, 2, 0.05, 0.35),
        Cue::Bet => chips(&mut out, &mut noise, 4, 0.045, 0.35),
        Cue::AllIn => {
            mix(&mut out, 0.0, &thump(85.0, 0.32, 0.7));
            chips(&mut out, &mut noise, 9, 0.038, 0.33);
        }
        Cue::Pot => chips(&mut out, &mut noise, 6, 0.06, 0.25),
        Cue::Win => {
            for (k, &f) in [523.25f32, 659.25, 783.99, 1046.5].iter().enumerate() {
                mix(&mut out, k as f32 * 0.1, &tone(f, 0.45, 0.25, 5.0));
            }
            chips(&mut out, &mut noise, 4, 0.05, 0.2);
        }
        Cue::Lose => {
            for (k, &f) in [392.0f32, 329.63, 261.63].iter().enumerate() {
                mix(&mut out, k as f32 * 0.2, &tone(f, 0.55, 0.25, 4.0));
            }
        }
        Cue::Bust => mix(&mut out, 0.0, &thump(70.0, 0.35, 0.6)),
        Cue::Fanfare => {
            for (k, &f) in [523.25f32, 659.25, 783.99].iter().enumerate() {
                mix(&mut out, k as f32 * 0.12, &tone(f, 0.3, 0.25, 6.0));
            }
            mix(&mut out, 0.4, &tone(1046.5, 1.1, 0.25, 2.5));
            mix(&mut out, 0.4, &tone(1318.5, 1.1, 0.18, 2.5));
            mix(&mut out, 0.4, &tone(783.99, 1.1, 0.15, 2.5));
        }
        Cue::Turn => {
            mix(&mut out, 0.0, &tone(659.25, 0.35, 0.2, 7.0));
            mix(&mut out, 0.09, &tone(880.0, 0.4, 0.2, 7.0));
        }
        Cue::Tick => mix(&mut out, 0.0, &click(1800.0, 0.025, 0.25, 220.0)),
    }
    normalize(&mut out, 0.9);
    out
}

// ------------------------------------------------------------------ playback

/// Plays the synthesized cues at the chosen volume.
pub struct Sound {
    bank: Vec<Vec<f32>>,
    volume: u8,
    unavailable: Option<&'static str>,
    /// The open audio device. Its mixer plays any number of sounds at once.
    #[cfg(feature = "sound")]
    output: Option<rodio::MixerDeviceSink>,
}

/// Opens the default audio output with rodio (0.23 API).
#[cfg(feature = "sound")]
fn open_output() -> Option<rodio::MixerDeviceSink> {
    // Stream errors (for example an unplugged headset) are ignored instead of being
    // printed to stderr, which would garble the terminal screen.
    let quiet = rodio::DeviceSinkBuilder::from_default_device()
        .and_then(|builder| builder.with_error_callback(|_| {}).open_stream());
    // Fall back to rodio's own search over the other output devices and formats.
    let mut sink = match quiet {
        Ok(sink) => sink,
        Err(_) => rodio::DeviceSinkBuilder::open_default_sink().ok()?,
    };
    // Otherwise rodio prints a notice to stderr when the device is closed at exit.
    sink.log_on_drop(false);
    Some(sink)
}

impl Sound {
    /// Opens the default audio output unless `muted`. Never fails: without an
    /// audio device the game simply stays silent.
    pub fn new(muted: bool) -> Sound {
        let bank: Vec<Vec<f32>> = Cue::ALL.iter().map(|&c| synthesize(c)).collect();

        #[cfg(feature = "sound")]
        let output = if muted { None } else { open_output() };

        #[cfg(feature = "sound")]
        let unavailable = if muted {
            Some("off (--mute)")
        } else if output.is_none() {
            Some("no audio device")
        } else {
            None
        };

        #[cfg(not(feature = "sound"))]
        let unavailable = {
            let _ = muted;
            Some("built without sound")
        };

        Sound {
            bank,
            volume: 60,
            unavailable,
            #[cfg(feature = "sound")]
            output,
        }
    }

    /// Sound that counts as working but plays nothing: for pictures of the
    /// screen, which should show the volume rather than "no sound".
    #[cfg(test)]
    pub fn silent() -> Sound {
        let mut sound = Sound::new(true);
        sound.unavailable = None;
        sound
    }

    /// `None` when sound works, otherwise a short reason why it does not.
    pub fn unavailable(&self) -> Option<&'static str> {
        self.unavailable
    }

    /// Volume from 0 (silent) to 100.
    pub fn volume(&self) -> u8 {
        self.volume
    }

    pub fn set_volume(&mut self, volume: u8) {
        self.volume = volume.min(100);
    }

    /// Starts playing `cue` after `delay_ms` milliseconds. Sounds overlap freely.
    pub fn play(&self, cue: Cue, delay_ms: u64) {
        if self.volume == 0 || self.unavailable.is_some() {
            return;
        }
        // Loudness is perceived roughly logarithmically; squaring feels even.
        let gain = (f32::from(self.volume) / 100.0).powi(2);
        let index = Cue::ALL.iter().position(|&c| c == cue).unwrap_or(0);
        self.output_play(&self.bank[index], gain, delay_ms);
    }

    #[cfg(feature = "sound")]
    fn output_play(&self, samples: &[f32], gain: f32, delay_ms: u64) {
        use rodio::Source;
        use std::num::{NonZeroU16, NonZeroU32};
        let Some(sink) = &self.output else {
            return;
        };
        let (Some(channels), Some(rate)) = (NonZeroU16::new(1), NonZeroU32::new(SAMPLE_RATE))
        else {
            return;
        };
        // `rodio::Sample` is f32 unless rodio's `64bit` feature is enabled.
        let data: Vec<rodio::Sample> = samples.iter().map(|&s| s as rodio::Sample).collect();
        let source = rodio::buffer::SamplesBuffer::new(channels, rate, data)
            .delay(std::time::Duration::from_millis(delay_ms))
            .amplify(gain as rodio::Float);
        sink.mixer().add(source);
    }

    #[cfg(not(feature = "sound"))]
    fn output_play(&self, _samples: &[f32], _gain: f32, _delay_ms: u64) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_cue_is_short_clean_and_audible() {
        for &cue in &Cue::ALL {
            let s = synthesize(cue);
            assert!(!s.is_empty(), "{cue:?} is empty");
            assert!(
                s.len() < 2 * SAMPLE_RATE as usize,
                "{cue:?} is longer than two seconds"
            );
            let peak = s.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            assert!(
                s.iter().all(|x| x.is_finite()),
                "{cue:?} has invalid samples"
            );
            assert!(peak <= 0.9 + 1e-6, "{cue:?} peaks at {peak}");
            assert!(peak > 0.05, "{cue:?} is nearly silent ({peak})");
            // Starts and ends quietly, so there is no click.
            assert!(s[0].abs() < 0.05, "{cue:?} starts with a click");
            assert!(s[s.len() - 1].abs() < 0.05, "{cue:?} ends with a click");
        }
    }

    #[test]
    fn cues_are_deterministic_and_distinct() {
        assert_eq!(synthesize(Cue::Bet), synthesize(Cue::Bet));
        assert_ne!(synthesize(Cue::Bet), synthesize(Cue::Chip));
        assert_ne!(synthesize(Cue::Win), synthesize(Cue::Lose));
    }

    #[test]
    fn muted_sound_never_plays_and_volume_is_clamped() {
        let mut s = Sound::new(true);
        assert!(s.unavailable().is_some());
        s.set_volume(250);
        assert_eq!(s.volume(), 100);
        s.play(Cue::Win, 0); // must be a harmless no-op
    }
}
