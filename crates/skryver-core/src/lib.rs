//! skryver-core — platform-portable dictation pipeline.
//!
//! Windows-first; platform-specific pieces (hotkey polling, paste injection)
//! live behind `cfg(windows)` and will grow trait seams when the macOS port lands.

pub mod asr;
pub mod audio;
pub mod hotkey;
pub mod inject;
pub mod llm;
pub mod postproc;
