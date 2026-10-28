//! Development hook for automated visual checks.
//!
//! `STORAGEMAP_SCREENSHOT=/tmp/shot.rgba storagemap PATH` scans PATH, waits
//! for the UI to settle, writes the window as `"W H\n"` + raw RGBA bytes and
//! exits. `STORAGEMAP_SCREENSHOT_TAB` picks the tab (e.g. `details`);
//! `STORAGEMAP_SCREENSHOT_SETTINGS` opens a settings page (e.g. `updates`).
//! Without a PATH it captures the start screen.
//! Inactive unless the variable is set.

use std::io::Write;
use std::path::PathBuf;

use egui::{Context, ScreenshotCallback, ViewportCommand};

use crate::views::Tab;

/// Frames to wait after the scan so layout and fonts are settled.
const SETTLE_FRAMES: u32 = 15;

pub struct DevShot {
    path: PathBuf,
    pub tab: Option<Tab>,
    /// Settings page to open, from `STORAGEMAP_SCREENSHOT_SETTINGS`.
    pub settings: Option<String>,
    settled: u32,
    requested: bool,
}

impl DevShot {
    pub fn from_env() -> Option<DevShot> {
        let path = PathBuf::from(std::env::var_os("STORAGEMAP_SCREENSHOT")?);
        let tab = std::env::var("STORAGEMAP_SCREENSHOT_TAB")
            .ok()
            .and_then(|name| {
                Tab::ALL
                    .into_iter()
                    .find(|t| t.label_id().trim_start_matches("tab-") == name)
            });
        Some(DevShot {
            path,
            tab,
            settings: std::env::var("STORAGEMAP_SCREENSHOT_SETTINGS").ok(),
            settled: 0,
            requested: false,
        })
    }

    /// Call once per frame; `ready` is true once the scan result is shown.
    pub fn frame(&mut self, ctx: &Context, ready: bool) {
        if self.requested {
            return;
        }
        ctx.request_repaint();
        if !ready {
            return;
        }
        self.settled += 1;
        if self.settled < SETTLE_FRAMES {
            return;
        }
        self.requested = true;
        let (path, closer) = (self.path.clone(), ctx.clone());
        ctx.send_viewport_cmd(ViewportCommand::Screenshot(ScreenshotCallback::new(
            move |image| {
                let [w, h] = image.size;
                let write = || -> std::io::Result<()> {
                    let mut f = std::io::BufWriter::new(std::fs::File::create(&path)?);
                    writeln!(f, "{w} {h}")?;
                    for px in &image.pixels {
                        f.write_all(&px.to_array())?;
                    }
                    f.flush()
                };
                if let Err(e) = write() {
                    tracing::error!(%e, path = %path.display(), "cannot write screenshot");
                }
                closer.send_viewport_cmd(ViewportCommand::Close);
                closer.request_repaint();
            },
        )));
    }
}
