//! `Sekai.Live2D.Live2DEyeBlinkController`.
//!
//! After `BuildModelComponent` the controller is put to sleep, and every body-motion change
//! calls `Sleep()` again, so in stories blinking is driven by clip events
//! (`OnLive2DInvokeUserData("eyeblink,closing,closed,opening")`). The command is a linear
//! tween: down to 0 over `closing`, hold `closed`, back to 1 over `opening`.

use sse_core::consts;

#[derive(Debug, Clone, Copy)]
struct Command {
    elapsed: f32,
    from: f32,
    closing: f32,
    closed: f32,
    opening: f32,
}

#[derive(Debug, Clone)]
pub struct EyeBlink {
    /// `CubismEyeBlinkController.EyeOpening`, multiplied into the eye parameters.
    pub opening: f32,
    command: Option<Command>,
}

impl Default for EyeBlink {
    fn default() -> Self {
        Self {
            opening: 1.0,
            command: None,
        }
    }
}

impl EyeBlink {
    /// Handles `OnLive2DInvokeUserData` data such as `eyeblink,0.2,0.79,0.24`.
    pub fn invoke_user_data(&mut self, data: &str) {
        let mut parts = data.split(',');
        if parts.next().map(str::trim) != Some("eyeblink") {
            return;
        }
        let mut arg = || {
            parts
                .next()
                .and_then(|p| p.trim().parse::<f32>().ok())
                .unwrap_or(consts::EYEBLINK_COMMAND_DEFAULT)
        };
        let (closing, closed, opening) = (arg(), arg(), arg());
        self.command = Some(Command {
            elapsed: 0.0,
            from: self.opening,
            closing,
            closed,
            opening,
        });
    }

    pub fn update(&mut self, delta: f32) {
        let Some(c) = &mut self.command else { return };
        c.elapsed += delta;
        let t = c.elapsed;
        self.opening = if t < c.closing {
            c.from + (0.0 - c.from) * (t / c.closing)
        } else if t < c.closing + c.closed {
            0.0
        } else if t < c.closing + c.closed + c.opening {
            (t - c.closing - c.closed) / c.opening
        } else {
            self.command = None;
            1.0
        };
    }
}
