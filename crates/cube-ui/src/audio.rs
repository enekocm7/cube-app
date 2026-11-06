//! Embedded sounds: the inspection warnings, using the same two cues as
//! cube-tui, and the personal best chime.

use std::io::Cursor;
use std::sync::OnceLock;

use cube_core::InspectionCue;
use rodio::{DeviceSinkBuilder, MixerDeviceSink};

const EIGHT_SECONDS_FILE: &[u8] = include_bytes!("../../../assets/audio/8-seconds.mp3");
const TWELVE_SECONDS_FILE: &[u8] = include_bytes!("../../../assets/audio/12-seconds.mp3");
const PERSONAL_BEST_FILE: &[u8] = include_bytes!("../../../assets/audio/personal-best.mp3");

static MIXER: OnceLock<Result<MixerDeviceSink, String>> = OnceLock::new();

/// Plays the embedded warning for an inspection cue.
pub fn play(cue: InspectionCue) -> anyhow::Result<()> {
    play_bytes(match cue {
        InspectionCue::EightSeconds => EIGHT_SECONDS_FILE,
        InspectionCue::TwelveSeconds => TWELVE_SECONDS_FILE,
    })
}

/// Plays the chime for a new personal best.
pub fn play_personal_best() -> anyhow::Result<()> {
    play_bytes(PERSONAL_BEST_FILE)
}

/// Plays an embedded sound. The output mixer is opened on the first call and
/// reused afterwards.
fn play_bytes(bytes: &'static [u8]) -> anyhow::Result<()> {
    let mixer = MIXER
        .get_or_init(|| DeviceSinkBuilder::open_default_sink().map_err(|error| error.to_string()))
        .as_ref()
        .map_err(|error| anyhow::anyhow!("Could not open audio output: {error}"))?
        .mixer();
    let player = rodio::play(mixer, Cursor::new(bytes))?;
    player.detach();
    Ok(())
}
