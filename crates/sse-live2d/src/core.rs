//! Cubism Core FFI (decision Q1). The only `unsafe` code in the workspace (determinism D-6).
//!
//! Safety model: a [`Moc`] owns a 64-byte aligned copy of the moc3 bytes and a [`Model`] owns
//! its 16-byte aligned model memory, and keeps its moc alive via `Arc`. All pointers
//! returned by Core point into these buffers and stay valid until the next `csmUpdateModel`
//! or the model is dropped; the safe accessors copy out of them immediately.
#![allow(unsafe_code)]

use std::ffi::CStr;
use std::os::raw::{c_char, c_float, c_int, c_uint, c_ushort, c_void};
use std::sync::Arc;

#[repr(C)]
struct CsmMoc {
    _p: [u8; 0],
}
#[repr(C)]
struct CsmModel {
    _p: [u8; 0],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CsmVector2 {
    x: c_float,
    y: c_float,
}

unsafe extern "C" {
    fn csmGetVersion() -> c_uint;
    fn csmGetMocVersion(address: *const c_void, size: c_uint) -> c_uint;
    fn csmReviveMocInPlace(address: *mut c_void, size: c_uint) -> *mut CsmMoc;
    fn csmGetSizeofModel(moc: *const CsmMoc) -> c_uint;
    fn csmInitializeModelInPlace(moc: *const CsmMoc, address: *mut c_void, size: c_uint)
    -> *mut CsmModel;
    fn csmUpdateModel(model: *mut CsmModel);
    fn csmReadCanvasInfo(
        model: *const CsmModel,
        size_in_pixels: *mut CsmVector2,
        origin_in_pixels: *mut CsmVector2,
        pixels_per_unit: *mut c_float,
    );
    fn csmGetRenderOrders(model: *const CsmModel) -> *const c_int;

    fn csmGetParameterCount(model: *const CsmModel) -> c_int;
    fn csmGetParameterIds(model: *const CsmModel) -> *const *const c_char;
    fn csmGetParameterMinimumValues(model: *const CsmModel) -> *const c_float;
    fn csmGetParameterMaximumValues(model: *const CsmModel) -> *const c_float;
    fn csmGetParameterDefaultValues(model: *const CsmModel) -> *const c_float;
    fn csmGetParameterValues(model: *mut CsmModel) -> *mut c_float;

    fn csmGetPartCount(model: *const CsmModel) -> c_int;
    fn csmGetPartIds(model: *const CsmModel) -> *const *const c_char;
    fn csmGetPartOpacities(model: *mut CsmModel) -> *mut c_float;

    fn csmGetDrawableCount(model: *const CsmModel) -> c_int;
    fn csmGetDrawableIds(model: *const CsmModel) -> *const *const c_char;
    fn csmGetDrawableConstantFlags(model: *const CsmModel) -> *const u8;
    fn csmGetDrawableDynamicFlags(model: *const CsmModel) -> *const u8;
    fn csmGetDrawableTextureIndices(model: *const CsmModel) -> *const c_int;
    fn csmGetDrawableOpacities(model: *const CsmModel) -> *const c_float;
    fn csmGetDrawableMaskCounts(model: *const CsmModel) -> *const c_int;
    fn csmGetDrawableMasks(model: *const CsmModel) -> *const *const c_int;
    fn csmGetDrawableVertexCounts(model: *const CsmModel) -> *const c_int;
    fn csmGetDrawableVertexPositions(model: *const CsmModel) -> *const *const CsmVector2;
    fn csmGetDrawableVertexUvs(model: *const CsmModel) -> *const *const CsmVector2;
    fn csmGetDrawableIndexCounts(model: *const CsmModel) -> *const c_int;
    fn csmGetDrawableIndices(model: *const CsmModel) -> *const *const c_ushort;
    fn csmResetDrawableDynamicFlags(model: *mut CsmModel);
}

/// `csmGetVersion()` as `(major, minor, patch)`.
pub fn core_version() -> (u32, u32, u32) {
    // SAFETY: no arguments, returns a plain integer.
    let v = unsafe { csmGetVersion() };
    (v >> 24, (v >> 16) & 0xFF, v & 0xFFFF)
}

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("moc3 is not valid or its version is not supported by this Core")]
    InvalidMoc,
    #[error("model initialisation failed")]
    InitFailed,
}

/// Heap buffer with a guaranteed alignment.
struct Aligned {
    buf: Vec<u8>,
    offset: usize,
    len: usize,
}

impl Aligned {
    fn new(len: usize, align: usize) -> Self {
        let buf = vec![0u8; len + align];
        let offset = buf.as_ptr().align_offset(align);
        Self { buf, offset, len }
    }

    fn as_mut_ptr(&mut self) -> *mut c_void {
        self.buf[self.offset..].as_mut_ptr().cast()
    }

    fn slice_mut(&mut self) -> &mut [u8] {
        &mut self.buf[self.offset..self.offset + self.len]
    }
}

pub struct Moc {
    _mem: Aligned,
    ptr: *mut CsmMoc,
    pub moc_version: u32,
}

// SAFETY: the moc is immutable after revival; Core only reads it.
unsafe impl Send for Moc {}
// SAFETY: see above.
unsafe impl Sync for Moc {}

impl Moc {
    pub fn new(bytes: &[u8]) -> Result<Arc<Self>, CoreError> {
        let mut mem = Aligned::new(bytes.len(), 64);
        mem.slice_mut().copy_from_slice(bytes);
        let size = bytes.len() as c_uint;
        // SAFETY: `mem` is 64-byte aligned, `size` bytes long and owned by the returned Moc.
        let (moc_version, ptr) = unsafe {
            let v = csmGetMocVersion(mem.as_mut_ptr(), size);
            (v, csmReviveMocInPlace(mem.as_mut_ptr(), size))
        };
        if ptr.is_null() {
            return Err(CoreError::InvalidMoc);
        }
        Ok(Arc::new(Self {
            _mem: mem,
            ptr,
            moc_version,
        }))
    }
}

/// Canvas geometry from `csmReadCanvasInfo`.
#[derive(Debug, Clone, Copy)]
pub struct Canvas {
    pub size_px: [f32; 2],
    pub origin_px: [f32; 2],
    pub pixels_per_unit: f32,
}

/// Static description of a drawable.
#[derive(Debug, Clone)]
pub struct DrawableInfo {
    pub id: String,
    pub texture: usize,
    pub additive: bool,
    pub multiply: bool,
    pub double_sided: bool,
    pub inverted_mask: bool,
    pub masks: Vec<usize>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u16>,
}

/// Per-update state of a drawable.
#[derive(Debug, Clone)]
pub struct DrawableFrame {
    pub visible: bool,
    pub opacity: f32,
    pub positions: Vec<[f32; 2]>,
}

pub struct Model {
    moc: Arc<Moc>,
    _mem: Aligned,
    ptr: *mut CsmModel,
    pub parameter_ids: Vec<String>,
    pub parameter_min: Vec<f32>,
    pub parameter_max: Vec<f32>,
    pub parameter_default: Vec<f32>,
    pub part_ids: Vec<String>,
    pub drawables: Vec<DrawableInfo>,
    pub canvas: Canvas,
}

// SAFETY: a Model is only mutated through `&mut self`; Core has no global state per model.
unsafe impl Send for Model {}

unsafe fn ids(ptr: *const *const c_char, n: usize) -> Vec<String> {
    (0..n)
        .map(|i| {
            // SAFETY: Core guarantees `n` valid NUL-terminated strings.
            unsafe { CStr::from_ptr(*ptr.add(i)) }
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

unsafe fn floats(ptr: *const c_float, n: usize) -> Vec<f32> {
    // SAFETY: Core guarantees `n` floats.
    unsafe { std::slice::from_raw_parts(ptr, n) }.to_vec()
}

impl Model {
    pub fn new(moc: Arc<Moc>) -> Result<Self, CoreError> {
        // SAFETY: `moc.ptr` is a revived moc kept alive by the Arc held in the Model.
        let size = unsafe { csmGetSizeofModel(moc.ptr) } as usize;
        let mut mem = Aligned::new(size, 16);
        // SAFETY: `mem` is 16-byte aligned and `size` bytes long.
        let ptr = unsafe { csmInitializeModelInPlace(moc.ptr, mem.as_mut_ptr(), size as c_uint) };
        if ptr.is_null() {
            return Err(CoreError::InitFailed);
        }
        // SAFETY: all getters below read arrays sized by the matching count, owned by `mem`.
        unsafe {
            let np = csmGetParameterCount(ptr) as usize;
            let nparts = csmGetPartCount(ptr) as usize;
            let nd = csmGetDrawableCount(ptr) as usize;
            let mut canvas = Canvas {
                size_px: [0.0; 2],
                origin_px: [0.0; 2],
                pixels_per_unit: 0.0,
            };
            let mut s = CsmVector2 { x: 0.0, y: 0.0 };
            let mut o = CsmVector2 { x: 0.0, y: 0.0 };
            csmReadCanvasInfo(ptr, &mut s, &mut o, &mut canvas.pixels_per_unit);
            canvas.size_px = [s.x, s.y];
            canvas.origin_px = [o.x, o.y];

            let cflags = std::slice::from_raw_parts(csmGetDrawableConstantFlags(ptr), nd);
            let tex = std::slice::from_raw_parts(csmGetDrawableTextureIndices(ptr), nd);
            let mask_counts = std::slice::from_raw_parts(csmGetDrawableMaskCounts(ptr), nd);
            let masks = csmGetDrawableMasks(ptr);
            let vcounts = std::slice::from_raw_parts(csmGetDrawableVertexCounts(ptr), nd);
            let uvs = csmGetDrawableVertexUvs(ptr);
            let icounts = std::slice::from_raw_parts(csmGetDrawableIndexCounts(ptr), nd);
            let indices = csmGetDrawableIndices(ptr);
            let dids = ids(csmGetDrawableIds(ptr), nd);
            let drawables = (0..nd)
                .map(|i| {
                    let m = std::slice::from_raw_parts(*masks.add(i), mask_counts[i] as usize);
                    let uv =
                        std::slice::from_raw_parts(*uvs.add(i), vcounts[i] as usize);
                    let idx = if icounts[i] > 0 {
                        std::slice::from_raw_parts(*indices.add(i), icounts[i] as usize).to_vec()
                    } else {
                        Vec::new()
                    };
                    DrawableInfo {
                        id: dids[i].clone(),
                        texture: tex[i] as usize,
                        additive: cflags[i] & 1 != 0,
                        multiply: cflags[i] & 2 != 0,
                        double_sided: cflags[i] & 4 != 0,
                        inverted_mask: cflags[i] & 8 != 0,
                        masks: m.iter().map(|&x| x as usize).collect(),
                        uvs: uv.iter().map(|v| [v.x, v.y]).collect(),
                        indices: idx,
                    }
                })
                .collect();
            Ok(Self {
                parameter_ids: ids(csmGetParameterIds(ptr), np),
                parameter_min: floats(csmGetParameterMinimumValues(ptr), np),
                parameter_max: floats(csmGetParameterMaximumValues(ptr), np),
                parameter_default: floats(csmGetParameterDefaultValues(ptr), np),
                part_ids: ids(csmGetPartIds(ptr), nparts),
                drawables,
                canvas,
                moc,
                _mem: mem,
                ptr,
            })
        }
    }

    pub fn moc(&self) -> &Arc<Moc> {
        &self.moc
    }

    pub fn parameter_index(&self, id: &str) -> Option<usize> {
        self.parameter_ids.iter().position(|p| p == id)
    }

    /// Writes all parameter values (clamped by Core on update) and part opacities, then
    /// runs `csmUpdateModel`.
    pub fn update(&mut self, params: &[f32], part_opacities: Option<&[f32]>) {
        // SAFETY: the value arrays have exactly parameter/part count elements.
        unsafe {
            let n = self.parameter_ids.len();
            let dst = std::slice::from_raw_parts_mut(csmGetParameterValues(self.ptr), n);
            dst.copy_from_slice(&params[..n]);
            if let Some(parts) = part_opacities {
                let np = self.part_ids.len();
                let dst = std::slice::from_raw_parts_mut(csmGetPartOpacities(self.ptr), np);
                dst.copy_from_slice(&parts[..np]);
            }
            csmResetDrawableDynamicFlags(self.ptr);
            csmUpdateModel(self.ptr);
        }
    }

    /// Copies out the drawable state after [`Model::update`].
    pub fn drawable_frames(&self) -> Vec<DrawableFrame> {
        let nd = self.drawables.len();
        // SAFETY: arrays sized by drawable count / vertex counts, valid until next update.
        unsafe {
            let dflags = std::slice::from_raw_parts(csmGetDrawableDynamicFlags(self.ptr), nd);
            let op = std::slice::from_raw_parts(csmGetDrawableOpacities(self.ptr), nd);
            let vcounts = std::slice::from_raw_parts(csmGetDrawableVertexCounts(self.ptr), nd);
            let pos = csmGetDrawableVertexPositions(self.ptr);
            (0..nd)
                .map(|i| DrawableFrame {
                    visible: dflags[i] & 1 != 0,
                    opacity: op[i],
                    positions: std::slice::from_raw_parts(*pos.add(i), vcounts[i] as usize)
                        .iter()
                        .map(|v| [v.x, v.y])
                        .collect(),
                })
                .collect()
        }
    }

    /// Drawable indices in render order (back to front).
    pub fn render_order(&self) -> Vec<usize> {
        let nd = self.drawables.len();
        // SAFETY: array of drawable count ints, valid until next update.
        let orders = unsafe { std::slice::from_raw_parts(csmGetRenderOrders(self.ptr), nd) };
        let mut idx: Vec<usize> = (0..nd).collect();
        idx.sort_by_key(|&i| orders[i]);
        idx
    }
}
