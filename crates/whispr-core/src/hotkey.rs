//! Hold-to-talk hotkey. M1: poll `GetAsyncKeyState` (~15 ms granularity) — simple
//! and reliable. M2 replaces this with an event-driven low-level keyboard hook so
//! the key can also be swallowed from the target app.

#[cfg(windows)]
pub struct HoldKey {
    vk: i32,
}

#[cfg(windows)]
impl HoldKey {
    /// `vk` is a Win32 virtual-key code, e.g. `0x78` = F9.
    pub fn new(vk: i32) -> Self {
        Self { vk }
    }

    pub fn is_down(&self) -> bool {
        // High bit set = key currently down.
        unsafe { (winapi::um::winuser::GetAsyncKeyState(self.vk) as u16 & 0x8000) != 0 }
    }

    /// Block until the key goes down (poll), respecting a stop flag.
    pub fn wait_down(&self, stop: &std::sync::atomic::AtomicBool) -> bool {
        loop {
            if stop.load(std::sync::atomic::Ordering::Relaxed) {
                return false;
            }
            if self.is_down() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
    }

    /// Block until the key is released.
    pub fn wait_up(&self) {
        while self.is_down() {
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
    }
}

pub const VK_F9: i32 = 0x78;
