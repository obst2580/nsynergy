/// Input capture via rdev (desktop only - rdev does not compile on Android).
#[cfg(not(target_os = "android"))]
pub mod capture;

pub mod clipboard;
pub mod config;
pub mod discovery;
pub mod event;
pub mod inject;
pub mod permissions;
pub mod platform;
pub mod protocol;
pub mod screen;
pub mod security;
pub mod touch;
