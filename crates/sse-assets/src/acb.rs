//! The parts of a CRI ACB (`@UTF` tables) the BGM player needs beyond the waveforms that
//! SekaiStoryRipper already exported: a cue's tracks with their local AISACs (vertical
//! layers, `BGM_VERTICAL`) and, for block-sequence cues (`ReferenceType` 8, interactive
//! BGMs), the blocks.
//!
//! Waveforms are named as SekaiStoryRipper names them: `<stem>.audio/e<MemoryAwbId>.wav`
//! next to the `.acb`.

use std::collections::BTreeMap;

#[derive(Debug, Clone)]
enum Val<'a> {
    Int(i64),
    Float(f64),
    Str(String),
    Data(&'a [u8]),
    None,
}

impl Val<'_> {
    fn int(&self) -> Option<i64> {
        match self {
            Val::Int(v) => Some(*v),
            _ => None,
        }
    }

    fn float(&self) -> Option<f64> {
        match self {
            Val::Float(v) => Some(*v),
            Val::Int(v) => Some(*v as f64),
            _ => None,
        }
    }

    fn data(&self) -> &[u8] {
        match self {
            Val::Data(d) => d,
            _ => &[],
        }
    }
}

type Row<'a> = BTreeMap<String, Val<'a>>;

fn be16(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(o..o + 2)?.try_into().ok()?))
}

fn be32(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(o..o + 4)?.try_into().ok()?))
}

/// Parses one `@UTF` table.
fn utf(buf: &[u8]) -> Option<Vec<Row<'_>>> {
    if buf.get(0..4)? != b"@UTF" {
        return None;
    }
    let base = 8usize;
    let rows_off = usize::from(be16(buf, base + 2)?);
    let str_off = be32(buf, base + 4)? as usize;
    let data_off = be32(buf, base + 8)? as usize;
    let ncols = usize::from(be16(buf, base + 16)?);
    let row_len = usize::from(be16(buf, base + 18)?);
    let nrows = be32(buf, base + 20)? as usize;
    let string_at = |rel: u32| -> String {
        let start = base + str_off + rel as usize;
        let end = buf[start..]
            .iter()
            .position(|&c| c == 0)
            .map_or(buf.len(), |e| start + e);
        String::from_utf8_lossy(&buf[start..end]).into_owned()
    };
    // (name, type, storage, const value)
    let read = |ty: u8, at: usize| -> Option<(Val<'_>, usize)> {
        Some(match ty {
            0 => (Val::Int(i64::from(*buf.get(at)?)), 1),
            1 => (Val::Int(i64::from(*buf.get(at)? as i8)), 1),
            2 => (Val::Int(i64::from(be16(buf, at)?)), 2),
            3 => (Val::Int(i64::from(be16(buf, at)? as i16)), 2),
            4 => (Val::Int(i64::from(be32(buf, at)?)), 4),
            5 => (Val::Int(i64::from(be32(buf, at)? as i32)), 4),
            6 | 7 => {
                let v = u64::from_be_bytes(buf.get(at..at + 8)?.try_into().ok()?);
                (Val::Int(v as i64), 8)
            }
            8 => (Val::Float(f64::from(f32::from_bits(be32(buf, at)?))), 4),
            9 => {
                let v = u64::from_be_bytes(buf.get(at..at + 8)?.try_into().ok()?);
                (Val::Float(f64::from_bits(v)), 8)
            }
            0xa => (Val::Str(string_at(be32(buf, at)?)), 4),
            0xb => {
                let (o, n) = (be32(buf, at)? as usize, be32(buf, at + 4)? as usize);
                let s = base + data_off + o;
                (Val::Data(buf.get(s..s + n).unwrap_or(&[])), 8)
            }
            _ => return None,
        })
    };
    let mut cols = Vec::with_capacity(ncols);
    let mut p = base + 24;
    for _ in 0..ncols {
        let flags = *buf.get(p)?;
        let name = string_at(be32(buf, p + 1)?);
        p += 5;
        let (ty, storage) = (flags & 0x0f, flags & 0xf0);
        let konst = if storage == 0x30 {
            let (v, n) = read(ty, p)?;
            p += n;
            Some(v)
        } else {
            None
        };
        cols.push((name, ty, storage, konst));
    }
    let mut rows = Vec::with_capacity(nrows);
    for r in 0..nrows {
        let mut rp = base + rows_off + r * row_len;
        let mut row = Row::new();
        for (name, ty, storage, konst) in &cols {
            let v = match (storage, konst) {
                (0x30, Some(k)) => k.clone(),
                (0x50, _) => {
                    let (v, n) = read(*ty, rp)?;
                    rp += n;
                    v
                }
                _ => Val::None,
            };
            row.insert(name.clone(), v);
        }
        rows.push(row);
    }
    Some(rows)
}

fn u16s(d: &[u8]) -> Vec<u16> {
    d.as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_be_bytes(*c))
        .collect()
}

/// A local AISAC's volume graph (`GraphTable` type 1): control → volume, piecewise linear.
#[derive(Debug, Clone, PartialEq)]
pub struct Aisac {
    pub control: String,
    pub default: f32,
    pub points: Vec<[f32; 2]>,
}

impl Aisac {
    pub fn volume(&self, control: f32) -> f32 {
        let p = &self.points;
        match p.len() {
            0 => 1.0,
            1 => p[0][1],
            _ => {
                if control <= p[0][0] {
                    return p[0][1];
                }
                for w in p.windows(2) {
                    if control <= w[1][0] {
                        let t = (control - w[0][0]) / (w[1][0] - w[0][0]).max(1e-9);
                        return w[0][1] + (w[1][1] - w[0][1]) * t;
                    }
                }
                p[p.len() - 1][1]
            }
        }
    }
}

/// One block of a block sequence.
#[derive(Debug, Clone)]
pub struct Block {
    /// Seconds (`Length` ms + `LengthUs` µs).
    pub length: f64,
    /// `PlaybackType` 1: the block repeats until a transition is requested.
    pub looping: bool,
    /// Extra plays for a non-looping block (`NumLoops`).
    pub repeats: u32,
    /// `TransitionTiming` 1: a requested transition waits for the next of
    /// `TransitionTimingValue` equal parts of the block (0: the block's end).
    pub grid: Option<u32>,
    /// Waveform per layer (the block's tracks, in sequence-track order).
    pub waves: Vec<Option<String>>,
}

#[derive(Debug, Clone)]
pub struct CueStructure {
    /// Local AISAC of each sequence track (layer).
    pub layers: Vec<Option<Aisac>>,
    /// Block sequences only.
    pub blocks: Vec<Block>,
    /// Sequence cues: the waveform of each track.
    pub waves: Vec<Option<String>>,
}

/// Reads a cue's structure from an ACB. `dir` is the library directory of the `.acb` and
/// `stem` its file stem (the audio files live in `<dir>/<stem>.audio/`).
pub fn cue_structure(acb: &[u8], cue_name: &str, dir: &str, stem: &str) -> Option<CueStructure> {
    let header = utf(acb)?.into_iter().next()?;
    let table = |k: &str| -> Vec<Row<'_>> {
        header
            .get(k)
            .map(|v| v.data())
            .and_then(utf)
            .unwrap_or_default()
    };
    let (cues, names) = (table("CueTable"), table("CueNameTable"));
    let index = names
        .iter()
        .find(|r| matches!(r.get("CueName"), Some(Val::Str(s)) if s == cue_name))
        .and_then(|r| r.get("CueIndex")?.int())? as usize;
    let cue = cues.get(index)?;
    let ref_type = cue.get("ReferenceType")?.int()?;
    let ref_index = cue.get("ReferenceIndex")?.int()? as usize;
    let (tracks, events, synths, waveforms) = (
        table("TrackTable"),
        table("TrackEventTable"),
        table("SynthTable"),
        table("WaveformTable"),
    );
    let (aisacs, graphs, control_names) = (
        table("AisacTable"),
        table("GraphTable"),
        table("AisacControlNameTable"),
    );
    let control_name = |id: i64| -> String {
        control_names
            .iter()
            .find(|r| r.get("AisacControlId").and_then(Val::int) == Some(id))
            .and_then(|r| match r.get("AisacControlName") {
                Some(Val::Str(s)) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_default()
    };
    let aisac_of = |track: usize| -> Option<Aisac> {
        let t = tracks.get(track)?;
        let &ai = u16s(t.get("LocalAisacs")?.data()).first()?;
        let a = aisacs.get(usize::from(ai))?;
        let default = if a.get("DefaultControlFlag").and_then(Val::int) == Some(1) {
            a.get("DefaultControl")?.float()? as f32
        } else {
            0.0
        };
        // the volume graph (type 1) among the AISAC's graphs
        let points = u16s(a.get("GraphIndexes")?.data())
            .into_iter()
            .filter_map(|g| graphs.get(usize::from(g)))
            .find(|g| g.get("Type").and_then(Val::int) == Some(1))
            .map(|g| {
                let xs: Vec<f32> = g
                    .get("Controls")
                    .map(|v| v.data())
                    .unwrap_or(&[])
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|c| f32::from_be_bytes(*c))
                    .collect();
                let ys = u16s(g.get("Destinations").map(|v| v.data()).unwrap_or(&[]));
                xs.into_iter()
                    .zip(ys)
                    .map(|(x, y)| [x, f32::from(y) / 10000.0])
                    .collect()
            })?;
        Some(Aisac {
            control: control_name(a.get("ControlId")?.int()?),
            default,
            points,
        })
    };
    // track → note-on (0x07D0, type 2 = synth) → synth → first waveform → its AWB id
    let wave_of = |track: usize| -> Option<String> {
        let t = tracks.get(track)?;
        let ev = t.get("EventIndex")?.int()?;
        let cmd = events
            .get(usize::try_from(ev).ok()?)?
            .get("Command")?
            .data();
        let mut o = 0;
        while o + 3 <= cmd.len() {
            let op = u16::from_be_bytes([cmd[o], cmd[o + 1]]);
            let n = usize::from(cmd[o + 2]);
            let arg = cmd.get(o + 3..o + 3 + n)?;
            if op == 0x07d0 && n >= 4 {
                let (kind, idx) = (be16(arg, 0)?, be16(arg, 2)?);
                let synth = if kind == 2 {
                    synths.get(usize::from(idx))?
                } else {
                    return None;
                };
                let refs = u16s(synth.get("ReferenceItems")?.data());
                let wf = refs.as_chunks::<2>().0.iter().find(|p| p[0] == 1)?[1];
                let id = waveforms.get(usize::from(wf))?.get("MemoryAwbId")?.int()?;
                return Some(format!("{dir}/{stem}.audio/e{id}.wav"));
            }
            o += 3 + n;
        }
        None
    };
    match ref_type {
        // block sequence
        8 => {
            let seq = table("BlockSequenceTable").into_iter().nth(ref_index)?;
            let seq_tracks = u16s(seq.get("TrackIndex")?.data());
            let blocks_idx = u16s(seq.get("BlockIndex")?.data());
            let block_rows = table("BlockTable");
            let layers = seq_tracks
                .iter()
                .map(|&t| aisac_of(usize::from(t)))
                .collect();
            let blocks = blocks_idx
                .iter()
                .filter_map(|&b| block_rows.get(usize::from(b)))
                .map(|b| {
                    let ms = b.get("Length").and_then(Val::int).unwrap_or(0) as f64;
                    let us = b.get("LengthUs").and_then(Val::int).unwrap_or(0) as f64;
                    let ptype = b.get("PlaybackType").and_then(Val::int).unwrap_or(0);
                    let loops = b.get("NumLoops").and_then(Val::int).unwrap_or(0);
                    let timing = b.get("TransitionTiming").and_then(Val::int).unwrap_or(0);
                    let value = b
                        .get("TransitionTimingValue")
                        .and_then(Val::int)
                        .unwrap_or(0);
                    Block {
                        length: ms / 1000.0 + us / 1_000_000.0,
                        looping: ptype == 1,
                        repeats: if ptype == 1 {
                            0
                        } else {
                            loops.clamp(0, 1000) as u32
                        },
                        grid: (timing == 1 && value > 0 && value < 65535).then_some(value as u32),
                        waves: u16s(b.get("TrackIndex").map(|v| v.data()).unwrap_or(&[]))
                            .into_iter()
                            .map(|t| wave_of(usize::from(t)))
                            .collect(),
                    }
                })
                .collect();
            Some(CueStructure {
                layers,
                blocks,
                waves: Vec::new(),
            })
        }
        // sequence
        3 => {
            let seq = table("SequenceTable").into_iter().nth(ref_index)?;
            let seq_tracks = u16s(seq.get("TrackIndex")?.data());
            Some(CueStructure {
                layers: seq_tracks
                    .iter()
                    .map(|&t| aisac_of(usize::from(t)))
                    .collect(),
                blocks: Vec::new(),
                waves: seq_tracks
                    .iter()
                    .map(|&t| wave_of(usize::from(t)))
                    .collect(),
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aisac_graph_is_piecewise_linear() {
        let a = Aisac {
            control: "BGM_VERTICAL".into(),
            default: 0.0,
            points: vec![[0.0, 1.0], [0.05, 0.0], [0.1, 1.0], [0.25, 0.0]],
        };
        assert_eq!(a.volume(0.0), 1.0);
        assert_eq!(a.volume(0.1), 1.0);
        assert!((a.volume(0.025) - 0.5).abs() < 1e-6);
        assert_eq!(a.volume(1.0), 0.0);
    }
}
