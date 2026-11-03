//! Embedded inspection warnings, using the same two cues as cube-tui.

use std::io::Cursor;
use std::sync::OnceLock;

use cube_core::InspectionCue;
use rodio::{DeviceSinkBuilder, MixerDeviceSink};

const EIGHT_SECONDS_FILE: &[u8] = include_bytes!("../../../assets/audio/8-seconds.mp3");
const TWELVE_SECONDS_FILE: &[u8] = include_bytes!("../../../assets/audio/12-seconds.mp3");

static MIXER: OnceLock<Result<MixerDeviceSink, String>> = OnceLock::new();

/// Plays the embedded warning for an inspection cue.
///
/// The output mixer is opened on the first call and reused afterwards.
pub fn play(cue: InspectionCue) -> anyhow::Result<()> {
    let mixer = MIXER
        .get_or_init(|| DeviceSinkBuilder::open_default_sink().map_err(|error| error.to_string()))
        .as_ref()
        .map_err(|error| anyhow::anyhow!("Could not open audio output: {error}"))?
        .mixer();
    let bytes = match cue {
        InspectionCue::EightSeconds => EIGHT_SECONDS_FILE,
        InspectionCue::TwelveSeconds => TWELVE_SECONDS_FILE,
    };
    let player = rodio::play(mixer, Cursor::new(bytes))?;
    player.detach();
    Ok(())
}
