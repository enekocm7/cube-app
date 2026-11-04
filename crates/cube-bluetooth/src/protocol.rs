//! The GAN timer's Bluetooth protocol, kept apart from the radio so it can be
//! tested with plain bytes.

use cube_core::HardwareState;
use uuid::Uuid;

/// The service holding both characteristics.
pub(crate) const SERVICE: Uuid = Uuid::from_u128(0x0000fff0_0000_1000_8000_00805f9b34fb);
/// Read once an attempt finishes, for its time.
pub(crate) const TIME_CHARACTERISTIC: Uuid =
    Uuid::from_u128(0x0000fff2_0000_1000_8000_00805f9b34fb);
/// Notifies every state change.
pub(crate) const STATE_CHARACTERISTIC: Uuid =
    Uuid::from_u128(0x0000fff5_0000_1000_8000_00805f9b34fb);

/// Where a state notification keeps the state.
const STATE_OFFSET: usize = 3;

/// What a state notification means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Update {
    State(HardwareState),
    /// The attempt is over and its time has to be read.
    Finished,
    /// The notification is too short to hold a state.
    Incomplete,
    Unsupported(u8),
}

/// Reads a state notification.
pub(crate) fn decode_update(value: &[u8]) -> Update {
    match value.get(STATE_OFFSET) {
        None => Update::Incomplete,
        Some(1) => Update::State(HardwareState::GetSet),
        Some(2) => Update::State(HardwareState::HandsOff),
        Some(3) => Update::State(HardwareState::Running),
        Some(4) => Update::State(HardwareState::Stopped),
        Some(5) => Update::State(HardwareState::Idle),
        Some(6) => Update::State(HardwareState::HandsOn),
        Some(7) => Update::Finished,
        Some(&state) => Update::Unsupported(state),
    }
}

/// Reads a time as milliseconds: minutes, seconds, then the milliseconds as a
/// little-endian `u16`.
pub(crate) fn decode_time(bytes: &[u8]) -> Option<u64> {
    let &[minutes, seconds, low, high, ..] = bytes else {
        return None;
    };
    Some(
        u64::from(minutes) * 60_000
            + u64::from(seconds) * 1_000
            + u64::from(u16::from_le_bytes([low, high])),
    )
}

/// Returns whether an advertised name belongs to a GAN timer.
pub(crate) fn is_timer_name(name: &str) -> bool {
    name.to_lowercase().contains("gan")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn update(state: u8) -> Update {
        decode_update(&[0xfe, 0x08, 0x01, state, 0, 0])
    }

    #[test]
    fn every_state_byte_decodes() {
        for (byte, state) in [
            (1, HardwareState::GetSet),
            (2, HardwareState::HandsOff),
            (3, HardwareState::Running),
            (4, HardwareState::Stopped),
            (5, HardwareState::Idle),
            (6, HardwareState::HandsOn),
        ] {
            assert_eq!(update(byte), Update::State(state));
        }
        assert_eq!(update(7), Update::Finished);
    }

    #[test]
    fn unknown_and_short_updates_are_reported() {
        assert_eq!(update(0), Update::Unsupported(0));
        assert_eq!(update(9), Update::Unsupported(9));
        assert_eq!(decode_update(&[1, 2, 3]), Update::Incomplete);
        assert_eq!(decode_update(&[]), Update::Incomplete);
    }

    #[test]
    fn times_decode_to_milliseconds() {
        assert_eq!(decode_time(&[1, 2, 232, 3]), Some(63_000));
        assert_eq!(decode_time(&[0, 8, 0xd4, 0x03]), Some(8_980));
        assert_eq!(decode_time(&[0, 8, 0xd4, 0x03, 9, 9, 9]), Some(8_980));
    }

    #[test]
    fn incomplete_times_are_rejected() {
        for bytes in [&[][..], &[1], &[1, 2], &[1, 2, 3]] {
            assert_eq!(decode_time(bytes), None);
        }
    }

    #[test]
    fn only_gan_names_count_as_timers() {
        assert!(is_timer_name("GAN-1a2b"));
        assert!(is_timer_name("gan timer"));
        assert!(!is_timer_name("Headphones"));
    }

    #[test]
    fn the_uuids_are_the_timers_own() {
        assert_eq!(SERVICE.to_string(), "0000fff0-0000-1000-8000-00805f9b34fb");
        assert_eq!(
            TIME_CHARACTERISTIC.to_string(),
            "0000fff2-0000-1000-8000-00805f9b34fb"
        );
        assert_eq!(
            STATE_CHARACTERISTIC.to_string(),
            "0000fff5-0000-1000-8000-00805f9b34fb"
        );
    }
}
