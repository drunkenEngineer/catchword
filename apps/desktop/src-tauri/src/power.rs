//! Whether the computer runs on battery, so indexing can wait for mains
//! power (IDX-9).

use std::time::Duration;

use crate::contract::PauseReason;

/// How often the power source is looked at.
pub const CHECK_EVERY: Duration = Duration::from_secs(10);

/// True on battery, false on mains power, None if it cannot be told.
#[cfg(windows)]
pub fn on_battery() -> Option<bool> {
    use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

    // SAFETY: SYSTEM_POWER_STATUS is plain numbers, for which zero is valid.
    let mut status: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
    // SAFETY: the pointer is to a valid, writable SYSTEM_POWER_STATUS.
    if unsafe { GetSystemPowerStatus(&mut status) } == 0 {
        return None;
    }
    match status.ACLineStatus {
        0 => Some(true),
        1 => Some(false),
        _ => None,
    }
}

#[cfg(not(windows))]
pub fn on_battery() -> Option<bool> {
    None
}

/// What a look at the power source asks of indexing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Pause,
    Resume,
    Nothing,
}

/// Indexing pauses when the computer goes on battery, and resumes when it
/// is plugged in again. It pauses only on the change, so a user who resumes
/// on battery is not overruled until the next time it is unplugged. A pause
/// for any other reason is left alone.
///
/// `before` and `now` are what `on_battery` said last time and says now;
/// `enabled` is the setting; `paused` is why indexing is paused, if it is.
pub fn step(
    before: Option<bool>,
    now: Option<bool>,
    enabled: bool,
    paused: Option<PauseReason>,
) -> Step {
    if paused == Some(PauseReason::Battery) {
        return if !enabled || now == Some(false) {
            Step::Resume
        } else {
            Step::Nothing
        };
    }
    if enabled && paused.is_none() && now == Some(true) && before != Some(true) {
        return Step::Pause;
    }
    Step::Nothing
}

#[cfg(test)]
mod tests {
    use super::*;

    const ON: bool = true;

    #[test]
    fn unplugging_pauses_and_plugging_in_resumes() {
        assert_eq!(step(Some(false), Some(true), ON, None), Step::Pause);
        // At start, on battery already.
        assert_eq!(step(None, Some(true), ON, None), Step::Pause);
        let battery = Some(PauseReason::Battery);
        assert_eq!(step(Some(true), Some(true), ON, battery), Step::Nothing);
        assert_eq!(step(Some(true), Some(false), ON, battery), Step::Resume);
        assert_eq!(step(Some(false), Some(false), ON, None), Step::Nothing);
    }

    #[test]
    fn a_resume_on_battery_holds_until_the_next_unplug() {
        // Resumed by the user while on battery: not paused again...
        assert_eq!(step(Some(true), Some(true), ON, None), Step::Nothing);
        // ...until plugged in and unplugged again.
        assert_eq!(step(Some(true), Some(false), ON, None), Step::Nothing);
        assert_eq!(step(Some(false), Some(true), ON, None), Step::Pause);
    }

    #[test]
    fn other_pauses_and_the_setting_are_respected() {
        for other in [
            PauseReason::You,
            PauseReason::LowDisk,
            PauseReason::NewerIndex,
            PauseReason::SafeMode,
        ] {
            assert_eq!(
                step(Some(false), Some(true), ON, Some(other)),
                Step::Nothing
            );
            assert_eq!(
                step(Some(true), Some(false), ON, Some(other)),
                Step::Nothing
            );
        }
        // Switched off: no pause, and a battery pause ends.
        assert_eq!(step(Some(false), Some(true), false, None), Step::Nothing);
        let battery = Some(PauseReason::Battery);
        assert_eq!(step(Some(true), Some(true), false, battery), Step::Resume);
        // Unknown power: nothing changes.
        assert_eq!(step(Some(false), None, ON, None), Step::Nothing);
        assert_eq!(step(Some(true), None, ON, battery), Step::Nothing);
    }

    #[test]
    fn the_power_source_can_be_asked() {
        // Whatever this computer runs on, asking does not fail.
        let _ = on_battery();
    }
}
