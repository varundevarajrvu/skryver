//! Text injection. M1 strategy: clipboard paste with snapshot/restore.
//! Snapshot clipboard text -> set transcript -> synthesize Ctrl+V -> restore.
//! Known limits (documented, by design): elevated targets need whispr elevated
//! too; terminals may prefer Shift+Insert (M4 adds per-target fallbacks).

use anyhow::{Context, Result};

#[cfg(windows)]
pub fn paste_text(text: &str) -> Result<()> {
    let mut clip = arboard::Clipboard::new().context("open clipboard")?;
    let saved = clip.get_text().ok(); // non-text clipboard contents: not restored in M1

    clip.set_text(text.to_string()).context("set clipboard")?;
    std::thread::sleep(std::time::Duration::from_millis(30));
    send_ctrl_v();
    // Give the target app time to read the clipboard before restoring.
    std::thread::sleep(std::time::Duration::from_millis(150));

    if let Some(old) = saved {
        let _ = clip.set_text(old);
    }
    Ok(())
}

#[cfg(windows)]
fn send_ctrl_v() {
    use winapi::um::winuser::{
        SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL,
    };

    unsafe fn key(vk: u16, up: bool) -> INPUT {
        let mut input: INPUT = unsafe { std::mem::zeroed() };
        input.type_ = INPUT_KEYBOARD;
        let ki = unsafe { input.u.ki_mut() };
        *ki = KEYBDINPUT {
            wVk: vk,
            wScan: 0,
            dwFlags: if up { KEYEVENTF_KEYUP } else { 0 },
            time: 0,
            dwExtraInfo: 0,
        };
        input
    }

    const VK_V: u16 = 0x56;
    unsafe {
        let mut seq = [
            key(VK_CONTROL as u16, false),
            key(VK_V, false),
            key(VK_V, true),
            key(VK_CONTROL as u16, true),
        ];
        SendInput(seq.len() as u32, seq.as_mut_ptr(), std::mem::size_of::<INPUT>() as i32);
    }
}
