//! Movie frames through an `ffmpeg` child process. Frames are requested by index at the
//! output frame rate; sequential requests read the pipe, anything else restarts with a seek.
//! The frame is cover-fitted to the output size (scaled to fill, centred crop).

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};

pub struct MovieDecoder {
    ffmpeg: PathBuf,
    width: u32,
    height: u32,
    fps: u32,
    stream: Option<Stream>,
}

struct Stream {
    file: PathBuf,
    child: Child,
    out: ChildStdout,
    /// Index of the frame held in `frame`.
    index: u32,
    frame: Vec<u8>,
    ended: bool,
}

impl Drop for Stream {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl MovieDecoder {
    pub fn new(ffmpeg: PathBuf, width: u32, height: u32, fps: u32) -> Self {
        Self { ffmpeg, width, height, fps, stream: None }
    }

    /// RGBA8 of frame `index` (at the output rate) of `file`; after the end, the last frame.
    pub fn frame(&mut self, file: &Path, index: u32) -> Result<&[u8], String> {
        let reuse = self
            .stream
            .as_ref()
            .is_some_and(|s| s.file == file && (s.index == index || s.index + 1 == index || (s.ended && index > s.index)));
        if !reuse {
            self.stream = Some(self.open(file, index)?);
        }
        let size = (self.width * self.height * 4) as usize;
        let s = self.stream.as_mut().expect("opened");
        while s.index < index && !s.ended {
            let mut buf = vec![0u8; size];
            match s.out.read_exact(&mut buf) {
                Ok(()) => {
                    s.frame = buf;
                    s.index += 1;
                }
                Err(_) => s.ended = true,
            }
        }
        Ok(&s.frame)
    }

    fn open(&self, file: &Path, index: u32) -> Result<Stream, String> {
        let (w, h) = (self.width, self.height);
        let start = index as f64 / self.fps as f64;
        let mut child = Command::new(&self.ffmpeg)
            .args(["-v", "error", "-ss", &format!("{start:.6}"), "-i"])
            .arg(file)
            .args([
                "-vf",
                &format!("scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h},fps={}", self.fps),
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgba",
                "-",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("{}: {e}", self.ffmpeg.display()))?;
        let out = child.stdout.take().ok_or("ffmpeg stdout")?;
        let size = (w * h * 4) as usize;
        let mut stream = Stream {
            file: file.to_owned(),
            child,
            out,
            // the first read yields `index`
            index: index.saturating_sub(1),
            frame: vec![0u8; size],
            ended: false,
        };
        let mut buf = vec![0u8; size];
        if stream.out.read_exact(&mut buf).is_ok() {
            stream.frame = buf;
            stream.index = index;
        } else {
            stream.ended = true;
        }
        Ok(stream)
    }
}
