//! The list of timers a scan has found, and which one is selected.

/// A timer found by a scan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    /// The platform's identifier, which stays the same while a timer's
    /// advertised name changes.
    pub id: String,
    pub name: String,
}

/// Why a scan ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScanError {
    NoAdapter,
    Failed(String),
}

/// The state of the device list.
#[derive(Debug, Default)]
pub struct Picker {
    devices: Vec<Device>,
    selected: usize,
    error: Option<ScanError>,
}

impl Picker {
    /// Forgets the last scan, ready for a new one.
    pub fn restart(&mut self) {
        *self = Self::default();
    }

    /// Adds a device, or refreshes it if it is listed. Returns whether the
    /// list changed: scans report each timer over and over.
    pub fn add(&mut self, device: Device) -> bool {
        match self.devices.iter_mut().find(|known| known.id == device.id) {
            Some(known) if *known == device => false,
            Some(known) => {
                *known = device;
                true
            }
            None => {
                self.devices.push(device);
                true
            }
        }
    }

    pub fn devices(&self) -> &[Device] {
        &self.devices
    }

    pub const fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn selected(&self) -> Option<&Device> {
        self.devices.get(self.selected)
    }

    /// Selects the device at `index`, if there is one.
    pub fn select(&mut self, index: usize) {
        if index < self.devices.len() {
            self.selected = index;
        }
    }

    /// Moves the selection up the list, stopping at the first device.
    pub const fn select_previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// Moves the selection down the list, stopping at the last device.
    pub fn select_next(&mut self) {
        self.select(self.selected + 1);
    }

    /// Records why the scan ended.
    pub fn fail(&mut self, error: ScanError) {
        self.error = Some(error);
    }

    pub const fn error(&self) -> Option<&ScanError> {
        self.error.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(id: &str, name: &str) -> Device {
        Device {
            id: id.to_owned(),
            name: name.to_owned(),
        }
    }

    #[test]
    fn repeated_reports_do_not_change_the_list() {
        let mut picker = Picker::default();
        assert!(picker.add(device("a", "GAN-1")));
        assert!(!picker.add(device("a", "GAN-1")));
        assert_eq!(picker.devices().len(), 1);
    }

    #[test]
    fn a_new_name_refreshes_the_entry() {
        let mut picker = Picker::default();
        picker.add(device("a", "GAN"));
        assert!(picker.add(device("a", "GAN-1")));
        assert_eq!(picker.devices(), [device("a", "GAN-1")]);
    }

    #[test]
    fn the_selection_stays_inside_the_list() {
        let mut picker = Picker::default();
        assert_eq!(picker.selected(), None);
        picker.select_next();
        assert_eq!(picker.selected_index(), 0);

        picker.add(device("a", "GAN-1"));
        picker.add(device("b", "GAN-2"));
        picker.select_previous();
        assert_eq!(picker.selected_index(), 0);
        picker.select_next();
        picker.select_next();
        assert_eq!(picker.selected(), Some(&device("b", "GAN-2")));

        picker.select(9);
        assert_eq!(picker.selected_index(), 1);
        picker.select(0);
        assert_eq!(picker.selected_index(), 0);
    }

    #[test]
    fn restarting_forgets_the_last_scan() {
        let mut picker = Picker::default();
        picker.add(device("a", "GAN-1"));
        picker.add(device("b", "GAN-2"));
        picker.select_next();
        picker.fail(ScanError::NoAdapter);

        picker.restart();
        assert_eq!(picker.devices(), []);
        assert_eq!(picker.selected_index(), 0);
        assert_eq!(picker.error(), None);
    }
}
