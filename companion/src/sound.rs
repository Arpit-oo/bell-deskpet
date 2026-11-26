//! Laptop-side sounds: tiny synthesized chimes (no audio files) played with
//! PlaySound, plus optional spoken lines through the Windows voice.

use std::collections::HashMap;
use std::f32::consts::PI;
use std::sync::OnceLock;

use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sound {
    Blip,       // pet says something
    Chime,      // task done
    Bell,       // reminder
    ClaudeWait, // Claude needs you
    ClaudeDone, // Claude finished
    Buzz,       // focus-mode warning
    Start,      // focus / pomodoro start
    Break,      // break time
}

const RATE: u32 = 22_050;

/// (frequency Hz, start s, length s, volume 0..1, bell-like?)
type Note = (f32, f32, f32, f32, bool);

fn notes(s: Sound) -> Vec<Note> {
    match s {
        Sound::Blip => vec![(880.0, 0.0, 0.07, 0.25, false), (1318.5, 0.06, 0.09, 0.22, false)],
        Sound::Chime => vec![
            (1046.5, 0.00, 0.35, 0.30, true),
            (1318.5, 0.09, 0.35, 0.30, true),
            (1568.0, 0.18, 0.35, 0.30, true),
            (2093.0, 0.27, 0.60, 0.28, true),
        ],
        Sound::Bell => vec![
            (1046.5, 0.0, 0.9, 0.32, true),
            (1568.0, 0.0, 0.9, 0.18, true),
            (1046.5, 0.45, 0.9, 0.28, true),
            (1568.0, 0.45, 0.9, 0.15, true),
        ],
        Sound::ClaudeWait => vec![(659.3, 0.0, 0.16, 0.30, true), (987.8, 0.15, 0.30, 0.30, true)],
        Sound::ClaudeDone => vec![
            (784.0, 0.0, 0.18, 0.28, true),
            (1046.5, 0.12, 0.22, 0.28, true),
            (1318.5, 0.24, 0.45, 0.26, true),
        ],
        Sound::Buzz => vec![(196.0, 0.0, 0.12, 0.35, false), (196.0, 0.18, 0.12, 0.35, false)],
        Sound::Start => vec![
            (523.3, 0.0, 0.14, 0.26, true),
            (784.0, 0.10, 0.14, 0.26, true),
            (1046.5, 0.20, 0.40, 0.26, true),
        ],
        Sound::Break => vec![
            (1046.5, 0.0, 0.25, 0.24, true),
            (784.0, 0.16, 0.25, 0.24, true),
            (659.3, 0.32, 0.55, 0.24, true),
        ],
    }
}

fn render(s: Sound) -> Vec<u8> {
    let ns = notes(s);
    let total = ns.iter().map(|n| n.1 + n.2).fold(0.0f32, f32::max) + 0.05;
    let len = (total * RATE as f32) as usize;
    let mut buf = vec![0f32; len];
    for (f, start, dur, vol, bell) in ns {
        let s0 = (start * RATE as f32) as usize;
        let n = (dur * RATE as f32) as usize;
        for i in 0..n {
            let t = i as f32 / RATE as f32;
            let attack = (t / 0.005).min(1.0);
            let env = if bell { (-t * 6.0 / dur).exp() } else { 1.0 - (t / dur) } * attack;
            let mut v = (2.0 * PI * f * t).sin();
            if bell {
                v += 0.25 * (2.0 * PI * f * 2.0 * t).sin(); // overtone for a glassy chime
            } else if f < 300.0 {
                v = v.signum() * 0.6 + v * 0.4; // buzzier
            }
            if let Some(o) = buf.get_mut(s0 + i) {
                *o += v * env * vol;
            }
        }
    }
    wav(&buf)
}

fn wav(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32000.0) as i16).to_le_bytes());
    }
    out
}

static BANK: OnceLock<HashMap<Sound, Vec<u8>>> = OnceLock::new();

fn bank() -> &'static HashMap<Sound, Vec<u8>> {
    BANK.get_or_init(|| {
        [
            Sound::Blip,
            Sound::Chime,
            Sound::Bell,
            Sound::ClaudeWait,
            Sound::ClaudeDone,
            Sound::Buzz,
            Sound::Start,
            Sound::Break,
        ]
        .into_iter()
        .map(|s| (s, render(s)))
        .collect()
    })
}

/// Plays asynchronously (a new sound cuts the previous one).
pub fn play(s: Sound) {
    if let Some(data) = bank().get(&s) {
        // The buffer lives in a static, so it outlives the async playback.
        unsafe {
            PlaySoundW(data.as_ptr() as *const u16, std::ptr::null_mut(), SND_MEMORY | SND_ASYNC | SND_NODEFAULT);
        }
    }
}

/// Speaks `text` with the built-in Windows voice, in the background.
pub fn speak(text: &str) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let clean: String = text.chars().filter(|c| !c.is_control()).collect::<String>().replace('\'', "''");
    let script = format!(
        "Add-Type -AssemblyName System.Speech; $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; \
         $s.Rate = 1; $s.Speak('{clean}')"
    );
    let _ = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}
