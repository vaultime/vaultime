// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Controller input. The idle timers of the desktop only see the keyboard and
//! mouse, so a game played on a controller would turn idle without this.

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

/// Time since the last controller input, `None` until there was some. The
/// tracker calls this once per tick, which also collects the input.
pub fn idle_duration() -> Option<Duration> {
    static LAST_INPUT: Mutex<Option<Instant>> = Mutex::new(None);

    let mut last_input = LAST_INPUT.lock().unwrap_or_else(PoisonError::into_inner);
    if imp::input_since_last_poll() {
        *last_input = Some(Instant::now());
    }
    last_input.map(|at| at.elapsed())
}

/// Short label for the controller detection, shown in diagnostics.
pub fn detection_strategy() -> &'static str {
    imp::STRATEGY
}

/// Controllers connected at the last check.
pub fn connected_count() -> usize {
    imp::connected_count()
}

#[cfg(windows)]
mod imp {
    use std::sync::{Mutex, Once, PoisonError};
    use std::thread;
    use std::time::Instant;

    use log::{info, warn};
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::UI::Input::XboxController::{
        XINPUT_GAMEPAD, XINPUT_GAMEPAD_LEFT_THUMB_DEADZONE, XINPUT_GAMEPAD_RIGHT_THUMB_DEADZONE,
        XINPUT_GAMEPAD_TRIGGER_THRESHOLD, XINPUT_STATE, XInputGetState, XUSER_MAX_COUNT,
    };

    use crate::constants::{CONTROLLER_SAMPLE_INTERVAL, POLL_INTERVAL};

    pub const STRATEGY: &str = "xinput";

    const SLOTS: usize = XUSER_MAX_COUNT as usize;

    struct Pads {
        slots: [Option<XINPUT_STATE>; SLOTS],
        input: bool,
    }

    static PADS: Mutex<Pads> = Mutex::new(Pads {
        slots: [None; SLOTS],
        input: false,
    });

    pub fn input_since_last_poll() -> bool {
        static SAMPLER: Once = Once::new();
        SAMPLER.call_once(|| {
            let spawned = thread::Builder::new()
                .name("vaultime-controllers".into())
                .spawn(sample_forever);
            if let Err(error) = spawned {
                warn!("could not start reading controllers: {error}");
            }
        });
        std::mem::take(&mut PADS.lock().unwrap_or_else(PoisonError::into_inner).input)
    }

    pub fn connected_count() -> usize {
        PADS.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .slots
            .iter()
            .flatten()
            .count()
    }

    /// `XInput` only reports the current state, so connected controllers are
    /// read often enough to catch short presses. Empty slots are checked once
    /// per tracking tick.
    fn sample_forever() {
        let mut last_probe: Option<Instant> = None;
        loop {
            let probe = last_probe.is_none_or(|at| at.elapsed() >= POLL_INTERVAL);
            if probe {
                last_probe = Some(Instant::now());
            }
            let connected = sample(probe);
            thread::sleep(if connected {
                CONTROLLER_SAMPLE_INTERVAL
            } else {
                POLL_INTERVAL
            });
        }
    }

    /// Reads the connected slots, and the empty ones as well with `probe`.
    /// Returns whether a controller is connected.
    fn sample(probe: bool) -> bool {
        let mut pads = PADS.lock().unwrap_or_else(PoisonError::into_inner);
        let mut input = false;
        for (index, slot) in (0..XUSER_MAX_COUNT).zip(pads.slots.iter_mut()) {
            if slot.is_none() && !probe {
                continue;
            }
            let mut state = XINPUT_STATE::default();
            // SAFETY: `state` is a valid XINPUT_STATE for XInput to fill.
            let connected = unsafe { XInputGetState(index, &raw mut state) } == ERROR_SUCCESS;
            if connected != slot.is_some() {
                let change = if connected {
                    "connected"
                } else {
                    "disconnected"
                };
                info!("controller {} {change}", index + 1);
            }
            if connected {
                input |= slot.is_some_and(|last| is_input(&last, &state));
                *slot = Some(state);
            } else {
                *slot = None;
            }
        }
        pads.input |= input;
        pads.slots.iter().any(Option::is_some)
    }

    /// The packet number changes with every change, stick noise included, so
    /// the buttons have to change or the controller has to leave its rest
    /// position.
    pub(super) fn is_input(last: &XINPUT_STATE, now: &XINPUT_STATE) -> bool {
        now.dwPacketNumber != last.dwPacketNumber
            && (now.Gamepad.wButtons != last.Gamepad.wButtons || !at_rest(&now.Gamepad))
    }

    fn at_rest(pad: &XINPUT_GAMEPAD) -> bool {
        pad.wButtons == 0
            && u16::from(pad.bLeftTrigger) <= XINPUT_GAMEPAD_TRIGGER_THRESHOLD
            && u16::from(pad.bRightTrigger) <= XINPUT_GAMEPAD_TRIGGER_THRESHOLD
            && within(
                pad.sThumbLX,
                pad.sThumbLY,
                XINPUT_GAMEPAD_LEFT_THUMB_DEADZONE,
            )
            && within(
                pad.sThumbRX,
                pad.sThumbRY,
                XINPUT_GAMEPAD_RIGHT_THUMB_DEADZONE,
            )
    }

    fn within(x: i16, y: i16, deadzone: u16) -> bool {
        let (x, y, deadzone) = (i64::from(x), i64::from(y), i64::from(deadzone));
        x * x + y * y <= deadzone * deadzone
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use std::collections::{BTreeMap, HashMap};
    use std::ffi::{OsStr, OsString};
    use std::fs::{self, File, OpenOptions};
    use std::io::{ErrorKind, Read};
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::OpenOptionsExt;
    use std::path::Path;
    use std::sync::{Mutex, PoisonError};

    use log::info;

    use crate::constants::{CONTROLLER_AXIS_MOVE_DIVISOR, CONTROLLER_EVENTS_PER_READ};

    pub const STRATEGY: &str = "evdev";

    const SYSFS_INPUT: &str = "/sys/class/input";
    const DEV_INPUT: &str = "/dev/input";

    // From linux/input-event-codes.h.
    const EV_KEY: u16 = 0x01;
    const EV_ABS: u16 = 0x03;
    const KEY_RELEASED: i32 = 0;
    const ABS_CNT: usize = 0x40;
    /// `BTN_JOYSTICK` up to the end of the `BTN_GAMEPAD` block. Keyboards and
    /// mice have none of these, so they are never opened.
    const CONTROLLER_BUTTONS: std::ops::Range<usize> = 0x120..0x140;

    // `_IOR` from asm-generic/ioctl.h, for `EVIOCGABS`.
    const IOC_READ: usize = 2;
    const IOC_DIR_SHIFT: usize = 30;
    const IOC_SIZE_SHIFT: usize = 16;
    const IOC_TYPE_SHIFT: usize = 8;
    const EVIOCGABS_BASE: usize = 0x40;

    const EVENT_SIZE: usize = size_of::<libc::input_event>();

    pub(super) enum Device {
        Controller(Controller),
        Other,
    }

    static DEVICES: Mutex<BTreeMap<OsString, Device>> = Mutex::new(BTreeMap::new());

    pub fn input_since_last_poll() -> bool {
        let mut devices = DEVICES.lock().unwrap_or_else(PoisonError::into_inner);
        poll(&mut devices, Path::new(SYSFS_INPUT), Path::new(DEV_INPUT))
    }

    pub fn connected_count() -> usize {
        count(&DEVICES.lock().unwrap_or_else(PoisonError::into_inner))
    }

    pub(super) fn count(devices: &BTreeMap<OsString, Device>) -> usize {
        devices
            .values()
            .filter(|device| matches!(device, Device::Controller(_)))
            .count()
    }

    /// Opens new controllers, forgets removed ones and reads what the rest
    /// queued since the last poll. Returns whether any of it was input.
    pub(super) fn poll(devices: &mut BTreeMap<OsString, Device>, sysfs: &Path, dev: &Path) -> bool {
        let names: Vec<OsString> = fs::read_dir(sysfs)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .map(|entry| entry.file_name())
                    .filter(|name| name.to_string_lossy().starts_with("event"))
                    .collect()
            })
            .unwrap_or_default();

        devices.retain(|name, _| names.contains(name));
        for name in names {
            devices
                .entry(name)
                .or_insert_with_key(|name| open(sysfs, dev, name));
        }

        let mut input = false;
        devices.retain(|name, device| {
            let Device::Controller(controller) = device else {
                return true;
            };
            match controller.drain() {
                Ok(seen) => {
                    input |= seen;
                    true
                }
                Err(error) => {
                    info!("stopped reading controller {}: {error}", name.display());
                    false
                }
            }
        });
        input
    }

    fn open(sysfs: &Path, dev: &Path, name: &OsStr) -> Device {
        let device = sysfs.join(name).join("device");
        let is_controller = fs::read_to_string(device.join("capabilities/key"))
            .is_ok_and(|keys| CONTROLLER_BUTTONS.clone().any(|bit| has_bit(&keys, bit)));
        if !is_controller {
            return Device::Other;
        }

        let path = dev.join(name);
        let label = fs::read_to_string(device.join("name")).unwrap_or_default();
        match OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&path)
        {
            Ok(file) => {
                let axes = fs::read_to_string(device.join("capabilities/abs"))
                    .map(|bitmap| axes(&file, &bitmap))
                    .unwrap_or_default();
                info!("reading controller {} ({})", path.display(), label.trim());
                Device::Controller(Controller { file, axes })
            }
            Err(error) => {
                info!(
                    "cannot read controller {} ({}): {error}",
                    path.display(),
                    label.trim()
                );
                Device::Other
            }
        }
    }

    /// Whether `bit` is set in a sysfs capability bitmap. The kernel prints it
    /// as hex words, the most significant first, without leading zero words.
    pub(super) fn has_bit(bitmap: &str, bit: usize) -> bool {
        let word_bits = usize::BITS as usize;
        bitmap
            .split_whitespace()
            .rev()
            .nth(bit / word_bits)
            .and_then(|word| usize::from_str_radix(word, 16).ok())
            .is_some_and(|word| (word >> (bit % word_bits)) & 1 == 1)
    }

    fn axes(file: &File, bitmap: &str) -> HashMap<u16, Axis> {
        (0..ABS_CNT)
            .filter(|&axis| has_bit(bitmap, axis))
            .filter_map(|axis| {
                let info = abs_info(file, axis)?;
                Some((
                    axis as u16,
                    Axis::new(info.value, info.minimum, info.maximum),
                ))
            })
            .collect()
    }

    fn abs_info(file: &File, axis: usize) -> Option<libc::input_absinfo> {
        let request = (IOC_READ << IOC_DIR_SHIFT)
            | (size_of::<libc::input_absinfo>() << IOC_SIZE_SHIFT)
            | (usize::from(b'E') << IOC_TYPE_SHIFT)
            | (EVIOCGABS_BASE + axis);
        let mut info = libc::input_absinfo {
            value: 0,
            minimum: 0,
            maximum: 0,
            fuzz: 0,
            flat: 0,
            resolution: 0,
        };
        // SAFETY: EVIOCGABS writes one input_absinfo into `info`, which lives
        // for the whole call.
        let result =
            unsafe { libc::ioctl(file.as_raw_fd(), request as libc::Ioctl, &raw mut info) };
        (result == 0).then_some(info)
    }

    pub(super) struct Controller {
        pub(super) file: File,
        pub(super) axes: HashMap<u16, Axis>,
    }

    impl Controller {
        /// Reads every queued event. Returns whether any of them was input.
        pub(super) fn drain(&mut self) -> std::io::Result<bool> {
            let mut buffer = [0_u8; EVENT_SIZE * CONTROLLER_EVENTS_PER_READ];
            let mut input = false;
            loop {
                match self.file.read(&mut buffer) {
                    Ok(0) => return Ok(input),
                    Ok(read) => {
                        for event in buffer[..read].as_chunks::<EVENT_SIZE>().0 {
                            input |= self.is_input(event);
                        }
                    }
                    Err(error) if error.kind() == ErrorKind::WouldBlock => return Ok(input),
                    Err(error) if error.kind() == ErrorKind::Interrupted => {}
                    Err(error) => return Err(error),
                }
            }
        }

        fn is_input(&mut self, event: &[u8]) -> bool {
            let (kind, code, value) = parse_event(event);
            match kind {
                EV_KEY => value != KEY_RELEASED,
                EV_ABS => self
                    .axes
                    .get_mut(&code)
                    .is_some_and(|axis| axis.moved_to(value)),
                _ => false,
            }
        }
    }

    /// Type, code and value of a raw `input_event`, its last eight bytes.
    fn parse_event(event: &[u8]) -> (u16, u16, i32) {
        let tail = &event[event.len() - 8..];
        (
            u16::from_ne_bytes([tail[0], tail[1]]),
            u16::from_ne_bytes([tail[2], tail[3]]),
            i32::from_ne_bytes([tail[4], tail[5], tail[6], tail[7]]),
        )
    }

    /// Sticks and triggers jitter, so an axis only counts once it travels a
    /// real distance from where it last counted.
    pub(super) struct Axis {
        anchor: i32,
        step: u32,
    }

    impl Axis {
        pub(super) fn new(value: i32, minimum: i32, maximum: i32) -> Self {
            Self {
                anchor: value,
                step: maximum.abs_diff(minimum) / CONTROLLER_AXIS_MOVE_DIVISOR,
            }
        }

        pub(super) fn moved_to(&mut self, value: i32) -> bool {
            let moved = value.abs_diff(self.anchor) > self.step;
            if moved {
                self.anchor = value;
            }
            moved
        }
    }

    /// One raw `input_event` as the kernel writes it.
    #[cfg(test)]
    pub(super) fn event(kind: u16, code: u16, value: i32) -> Vec<u8> {
        let mut bytes = vec![0_u8; EVENT_SIZE];
        let tail = EVENT_SIZE - 8;
        bytes[tail..tail + 2].copy_from_slice(&kind.to_ne_bytes());
        bytes[tail + 2..tail + 4].copy_from_slice(&code.to_ne_bytes());
        bytes[tail + 4..].copy_from_slice(&value.to_ne_bytes());
        bytes
    }

    #[cfg(test)]
    pub(super) const TEST_EV_KEY: u16 = EV_KEY;
    #[cfg(test)]
    pub(super) const TEST_EV_ABS: u16 = EV_ABS;
}

#[cfg(not(any(windows, target_os = "linux")))]
mod imp {
    pub const STRATEGY: &str = "none";

    pub fn input_since_last_poll() -> bool {
        false
    }

    pub fn connected_count() -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    /// Prints what the controller detection sees. Press a button while it
    /// waits: `cargo test -- --ignored --nocapture report_controllers`.
    #[test]
    #[ignore = "reads the controllers of this PC"]
    fn report_controllers() {
        for _ in 0..3 {
            let idle = super::idle_duration();
            println!(
                "{}: {} connected, idle {idle:?}",
                super::detection_strategy(),
                super::connected_count()
            );
            std::thread::sleep(crate::constants::POLL_INTERVAL);
        }
    }

    #[cfg(windows)]
    #[test]
    fn xinput_counts_presses_and_real_moves_only() {
        use windows_sys::Win32::UI::Input::XboxController::{XINPUT_GAMEPAD_A, XINPUT_STATE};

        let rest = XINPUT_STATE::default();
        let next = |change: fn(&mut XINPUT_STATE)| {
            let mut state = rest;
            state.dwPacketNumber = 2;
            change(&mut state);
            state
        };

        // Noise inside the deadzone changes the packet but is no input.
        assert!(!super::imp::is_input(
            &rest,
            &next(|state| state.Gamepad.sThumbLX = 1_200)
        ));
        assert!(super::imp::is_input(
            &rest,
            &next(|state| state.Gamepad.sThumbLX = 20_000)
        ));
        assert!(super::imp::is_input(
            &rest,
            &next(|state| state.Gamepad.bRightTrigger = 200)
        ));
        assert!(super::imp::is_input(
            &rest,
            &next(|state| state.Gamepad.wButtons = XINPUT_GAMEPAD_A)
        ));

        // Letting go of a button is input as well.
        let pressed = next(|state| state.Gamepad.wButtons = XINPUT_GAMEPAD_A);
        let mut released = rest;
        released.dwPacketNumber = 3;
        assert!(super::imp::is_input(&pressed, &released));

        // A held button without a new packet is nothing new.
        assert!(!super::imp::is_input(&pressed, &pressed));
    }

    #[cfg(target_os = "linux")]
    mod linux {
        use std::collections::{BTreeMap, HashMap};
        use std::fs::{self, OpenOptions};
        use std::io::Write;
        use std::path::{Path, PathBuf};

        use super::super::imp::{
            Axis, Controller, TEST_EV_ABS, TEST_EV_KEY, count, event, has_bit, poll,
        };

        const BTN_SOUTH: usize = 0x130;
        const BTN_LEFT: usize = 0x110;
        const KEY_A: usize = 30;
        const ABS_X: u16 = 0x00;
        // The key bitmap of an Xbox controller with the xpad driver.
        const XPAD_KEYS: &str = "7cdb000000000000 0 0 0 0";
        const KEYBOARD_KEYS: &str =
            "1000000000007 ff9f207ac14057ff febeffdfffefffff fffffffffffffffe";

        #[test]
        fn reads_capability_bitmaps() {
            assert!(has_bit(XPAD_KEYS, BTN_SOUTH));
            assert!(!has_bit(XPAD_KEYS, BTN_LEFT));
            assert!(!has_bit(XPAD_KEYS, KEY_A));
            assert!(has_bit(KEYBOARD_KEYS, KEY_A));
            assert!(!has_bit(KEYBOARD_KEYS, BTN_SOUTH));
            assert!(!has_bit("", KEY_A));
        }

        #[test]
        fn axes_ignore_jitter() {
            let mut axis = Axis::new(0, -32_768, 32_767);
            assert!(!axis.moved_to(300));
            assert!(!axis.moved_to(-2_000));
            assert!(axis.moved_to(20_000));
            assert!(!axis.moved_to(19_000));
            assert!(axis.moved_to(0));

            // A hat only knows -1, 0 and 1, every change counts.
            let mut hat = Axis::new(0, -1, 1);
            assert!(hat.moved_to(1));
            assert!(hat.moved_to(0));
        }

        fn device(sysfs: &Path, dev: &Path, name: &str, keys: &str, events: &[Vec<u8>]) -> PathBuf {
            let capabilities = sysfs.join(name).join("device/capabilities");
            fs::create_dir_all(&capabilities).unwrap();
            fs::write(capabilities.join("key"), keys).unwrap();
            let path = dev.join(name);
            fs::write(&path, events.concat()).unwrap();
            path
        }

        fn append(path: &Path, events: &[Vec<u8>]) {
            let mut file = OpenOptions::new().append(true).open(path).unwrap();
            file.write_all(&events.concat()).unwrap();
        }

        #[test]
        fn polls_controllers_and_leaves_keyboards_alone() {
            let root = std::env::temp_dir()
                .join(format!("vaultime-controller-test-{}", uuid::Uuid::new_v4()));
            let (sysfs, dev) = (root.join("sys"), root.join("dev"));
            fs::create_dir_all(&dev).unwrap();
            let press = event(TEST_EV_KEY, BTN_SOUTH as u16, 1);
            let release = event(TEST_EV_KEY, BTN_SOUTH as u16, 0);
            let pad = device(
                &sysfs,
                &dev,
                "event7",
                XPAD_KEYS,
                std::slice::from_ref(&release),
            );
            let keyboard = device(
                &sysfs,
                &dev,
                "event3",
                KEYBOARD_KEYS,
                std::slice::from_ref(&press),
            );

            let mut devices = BTreeMap::new();
            assert!(
                !poll(&mut devices, &sysfs, &dev),
                "a key press on the keyboard is not read"
            );
            assert_eq!(count(&devices), 1);

            append(&keyboard, std::slice::from_ref(&press));
            assert!(!poll(&mut devices, &sysfs, &dev));
            append(&pad, &[press, release]);
            assert!(poll(&mut devices, &sysfs, &dev));
            assert!(
                !poll(&mut devices, &sysfs, &dev),
                "the queue is empty again"
            );

            fs::remove_dir_all(sysfs.join("event7")).unwrap();
            assert!(!poll(&mut devices, &sysfs, &dev));
            assert_eq!(count(&devices), 0);
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn counts_stick_moves_past_the_noise() {
            let path = std::env::temp_dir()
                .join(format!("vaultime-controller-axis-{}", uuid::Uuid::new_v4()));
            fs::write(
                &path,
                [
                    event(TEST_EV_ABS, ABS_X, 400),
                    event(TEST_EV_ABS, ABS_X, -900),
                ]
                .concat(),
            )
            .unwrap();
            let mut controller = Controller {
                file: fs::File::open(&path).unwrap(),
                axes: HashMap::from([(ABS_X, Axis::new(0, -32_768, 32_767))]),
            };
            assert!(!controller.drain().unwrap());

            append(&path, &[event(TEST_EV_ABS, ABS_X, 25_000)]);
            assert!(controller.drain().unwrap());
            fs::remove_file(path).unwrap();
        }
    }
}
