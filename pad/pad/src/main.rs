//! Fullscreen PIN pad for reMarkable Profiles.
//!
//! Runs at boot (via `rm-profile-pad.service`) before xochitl. Draws a numeric
//! keypad, reads the entered PIN, and asks `rmprofile-core` what to do:
//!   - unknown PIN  -> clear and let the user retry
//!   - active PIN   -> start xochitl (no switch)
//!   - other PIN    -> `rm-profile switch <name>`, then start xochitl
//!
//! All decision logic lives in `rmprofile-core` (host-tested). This binary is
//! only the framebuffer/input shell and the command execution, which can only
//! be exercised on-device.
//!
//! reMarkable 2 note: the display has no kernel framebuffer; libremarkable talks
//! to the `rm2fb` shim, so the pad must run with the rm2fb client preloaded
//! (see docs/INSTALL.md). This is convenience + basic privacy, not security.

use std::process::Command;
use std::sync::mpsc::channel;

use libremarkable::framebuffer::cgmath::{Point2, Vector2};
use libremarkable::framebuffer::common::{
    color, display_temp, dither_mode, waveform_mode, DRAWING_QUANT_BIT,
};
use libremarkable::framebuffer::{
    core::Framebuffer, FramebufferDraw, FramebufferRefresh,
};
use libremarkable::input::{ev::EvDevContext, InputDevice, InputEvent, MultitouchEvent};

use rmprofile_core::{decide, parse_pins, Action, PinEntry};

const PROFILES_DIR: &str = "/home/root/profiles";

/// A drawn key: its label and screen rectangle (x, y, w, h).
struct Key {
    label: &'static str,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

impl Key {
    fn contains(&self, px: u32, py: u32) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }
}

/// Read the active profile name from the `active` symlink.
fn active_profile() -> String {
    std::fs::read_link(format!("{PROFILES_DIR}/active"))
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default()
}

fn load_pins() -> Vec<PinEntry> {
    std::fs::read_to_string(format!("{PROFILES_DIR}/pins.conf"))
        .map(|c| parse_pins(&c))
        .unwrap_or_default()
}

fn start_xochitl() {
    let _ = Command::new("systemctl").arg("start").arg("xochitl").status();
}

fn switch_profile(name: &str) {
    let _ = Command::new("/home/root/profiles/rm-profile")
        .arg("switch")
        .arg(name)
        .status();
}

/// Build the keypad layout for a given screen size.
fn build_keys(screen_w: u32, screen_h: u32) -> Vec<Key> {
    let labels = [
        "1", "2", "3", "4", "5", "6", "7", "8", "9", "CLR", "0", "OK",
    ];
    let cols = 3;
    let rows = 4;
    let gap = 24;
    let grid_w = screen_w * 3 / 5;
    let grid_h = screen_h / 2;
    let key_w = (grid_w - gap * (cols - 1)) / cols;
    let key_h = (grid_h - gap * (rows - 1)) / rows;
    let ox = (screen_w - grid_w) / 2;
    let oy = screen_h - grid_h - gap * 4;

    let mut keys = Vec::with_capacity(labels.len());
    for (i, label) in labels.iter().enumerate() {
        let r = i as u32 / cols;
        let c = i as u32 % cols;
        keys.push(Key {
            label,
            x: ox + c * (key_w + gap),
            y: oy + r * (key_h + gap),
            w: key_w,
            h: key_h,
        });
    }
    keys
}

fn draw_ui(fb: &mut Framebuffer, keys: &[Key], entered_len: usize, screen_w: u32) {
    fb.clear();
    // Title.
    fb.draw_text(
        Point2 { x: 120.0, y: 160.0 },
        "reMarkable Profiles",
        60.0,
        color::BLACK,
        false,
    );
    // PIN dots.
    let dots: String = "* ".repeat(entered_len);
    fb.draw_text(
        Point2 { x: 120.0, y: 260.0 },
        &dots,
        70.0,
        color::BLACK,
        false,
    );
    // Keys.
    for key in keys {
        fb.draw_rect(
            Point2 { x: key.x as i32, y: key.y as i32 },
            Vector2 { x: key.w, y: key.h },
            3,
            color::BLACK,
        );
        fb.draw_text(
            Point2 {
                x: (key.x + key.w / 2 - 12) as f32,
                y: (key.y + key.h / 2 + 12) as f32,
            },
            key.label,
            48.0,
            color::BLACK,
            false,
        );
    }
    let _ = screen_w;
    fb.full_refresh(
        waveform_mode::WAVEFORM_MODE_GC16,
        display_temp::TEMP_USE_MAX,
        dither_mode::EPDC_FLAG_USE_DITHERING_PASSTHROUGH,
        DRAWING_QUANT_BIT,
        true,
    );
}

fn apply(action: Action) -> bool {
    match action {
        Action::Reject => false,
        Action::StartCurrent => {
            start_xochitl();
            true
        }
        Action::SwitchTo(name) => {
            switch_profile(&name);
            start_xochitl();
            true
        }
    }
}

fn main() {
    let mut fb = Framebuffer::new();
    let (screen_w, screen_h) = {
        let var = fb.var_screen_info.clone();
        (var.xres, var.yres)
    };
    let keys = build_keys(screen_w, screen_h);

    let mut entered = String::new();
    draw_ui(&mut fb, &keys, entered.len(), screen_w);

    let (tx, rx) = channel::<InputEvent>();
    let mut ctx = EvDevContext::new(InputDevice::Multitouch, tx);
    ctx.start();

    while let Ok(event) = rx.recv() {
        if let InputEvent::MultitouchEvent { event } = event {
            if let MultitouchEvent::Press { finger } = event {
                let (px, py) = (finger.pos.x as u32, finger.pos.y as u32);
                if let Some(key) = keys.iter().find(|k| k.contains(px, py)) {
                    match key.label {
                        "CLR" => entered.clear(),
                        "OK" => {
                            let action = decide(&load_pins(), &active_profile(), &entered);
                            if apply(action) {
                                break; // xochitl is starting; the pad's job is done.
                            }
                            entered.clear();
                        }
                        digit => {
                            if entered.len() < 8 {
                                entered.push_str(digit);
                            }
                        }
                    }
                    draw_ui(&mut fb, &keys, entered.len(), screen_w);
                }
            }
        }
    }
}
