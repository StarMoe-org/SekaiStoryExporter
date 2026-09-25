//! Cubism Core FFI (ADR-0003). The only `unsafe` code in the workspace (determinism D-6).
//!
//! Core is proprietary and never shipped with sse, so it is loaded at run time (`dlopen` /
//! `LoadLibrary`) from the first of: `SSE_CUBISM_CORE`, `SSE_CUBISM_CORE_DIR`, the directory
//! of the executable, the platform's library search path. Building needs no SDK.
//!
//! Safety model: a [`Moc`] owns a 64-byte aligned copy of the moc3 bytes and a [`Model`] owns
//! its 16-byte aligned model memory, and keeps its moc alive via `Arc`. All pointers
//! returned by Core point into these buffers and stay valid until the next `csmUpdateModel`
//! or the model is dropped; the safe accessors copy out of them immediately.
#![allow(unsafe_code)]

use std::ffi::{CStr, OsString};
use std::os::raw::{c_char, c_float, c_int, c_uint, c_ushort, c_void};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use libloading::Library;

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

/// Declares the Core entry points once: the `Api` table of function pointers and its loader.
macro_rules! api {
    ($($name:ident: fn($($arg:ty),*) $(-> $ret:ty)?;)*) => {
        #[allow(non_snake_case)]
        struct Api {
            $($name: unsafe extern "C" fn($($arg),*) $(-> $ret)?,)*
            _lib: Library,
        }

        impl Api {
            /// # Safety
            /// `lib` must be a Cubism Core build whose exports have the signatures above.
            unsafe fn load(lib: Library) -> Result<Self, libloading::Error> {
                // SAFETY: the caller vouches for the signatures; the pointers stay valid while
                // `_lib` is alive, and it lives as long as the table.
                unsafe {
                    Ok(Self {
                        $($name: *lib.get::<unsafe extern "C" fn($($arg),*) $(-> $ret)?>(
                            concat!(stringify!($name), "\0").as_bytes(),
                        )?,)*
                        _lib: lib,
                    })
                }
            }
        }
    };
}

api! {
    csmGetVersion: fn() -> c_uint;
    csmGetMocVersion: fn(*const c_void, c_uint) -> c_uint;
    csmReviveMocInPlace: fn(*mut c_void, c_uint) -> *mut CsmMoc;
    csmGetSizeofModel: fn(*const CsmMoc) -> c_uint;
    csmInitializeModelInPlace: fn(*const CsmMoc, *mut c_void, c_uint) -> *mut CsmModel;
    csmUpdateModel: fn(*mut CsmModel);
    csmReadCanvasInfo: fn(*const CsmModel, *mut CsmVector2, *mut CsmVector2, *mut c_float);
    csmGetRenderOrders: fn(*const CsmModel) -> *const c_int;

    csmGetParameterCount: fn(*const CsmModel) -> c_int;
    csmGetParameterIds: fn(*const CsmModel) -> *const *const c_char;
    csmGetParameterMinimumValues: fn(*const CsmModel) -> *const c_float;
    csmGetParameterMaximumValues: fn(*const CsmModel) -> *const c_float;
    csmGetParameterDefaultValues: fn(*const CsmModel) -> *const c_float;
    csmGetParameterValues: fn(*mut CsmModel) -> *mut c_float;

    csmGetPartCount: fn(*const CsmModel) -> c_int;
    csmGetPartIds: fn(*const CsmModel) -> *const *const c_char;
    csmGetPartOpacities: fn(*mut CsmModel) -> *mut c_float;

    csmGetDrawableCount: fn(*const CsmModel) -> c_int;
    csmGetDrawableIds: fn(*const CsmModel) -> *const *const c_char;
    csmGetDrawableConstantFlags: fn(*const CsmModel) -> *const u8;
    csmGetDrawableDynamicFlags: fn(*const CsmModel) -> *const u8;
    csmGetDrawableTextureIndices: fn(*const CsmModel) -> *const c_int;
    csmGetDrawableOpacities: fn(*const CsmModel) -> *const c_float;
    csmGetDrawableMaskCounts: fn(*const CsmModel) -> *const c_int;
    csmGetDrawableMasks: fn(*const CsmModel) -> *const *const c_int;
    csmGetDrawableVertexCounts: fn(*const CsmModel) -> *const c_int;
    csmGetDrawableVertexPositions: fn(*const CsmModel) -> *const *const CsmVector2;
    csmGetDrawableVertexUvs: fn(*const CsmModel) -> *const *const CsmVector2;
    csmGetDrawableIndexCounts: fn(*const CsmModel) -> *const c_int;
    csmGetDrawableIndices: fn(*const CsmModel) -> *const *const c_ushort;
    csmResetDrawableDynamicFlags: fn(*mut CsmModel);
}

/// Environment variable naming the Core shared library (or the directory holding it).
pub const CORE_ENV: &str = "SSE_CUBISM_CORE";
/// Environment variable naming the SDK's `Core/` directory; the platform's library under
/// `dll/` is used.
pub const CORE_DIR_ENV: &str = "SSE_CUBISM_CORE_DIR";

/// Where the SDK for Native keeps this platform's shared library, relative to `Core/`.
fn sdk_subpath() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", _) => Some("dll/macos/libLive2DCubismCore.dylib"),
        ("windows", "x86_64") => Some("dll/windows/x86_64/Live2DCubismCore.dll"),
        ("windows", "x86") => Some("dll/windows/x86/Live2DCubismCore.dll"),
        ("linux", "x86_64") => Some("dll/linux/x86_64/libLive2DCubismCore.so"),
        _ => None,
    }
}

/// Candidate locations, in order: `SSE_CUBISM_CORE`, `SSE_CUBISM_CORE_DIR`, next to the
/// executable, then the platform's library search path.
fn candidates() -> Vec<OsString> {
    let file = libloading::library_filename("Live2DCubismCore");
    let mut out = Vec::new();
    if let Some(p) = std::env::var_os(CORE_ENV).map(PathBuf::from) {
        out.push(if p.is_dir() { p.join(&file) } else { p }.into_os_string());
    }
    if let (Some(dir), Some(sub)) = (std::env::var_os(CORE_DIR_ENV), sdk_subpath()) {
        out.push(PathBuf::from(dir).join(sub).into_os_string());
    }
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_owned))
    {
        out.push(dir.join(&file).into_os_string());
    }
    out.push(file);
    out
}

fn load() -> Result<Api, String> {
    let mut tried = Vec::new();
    for path in candidates() {
        // SAFETY: loading runs the library's initialisers; Cubism Core has none with side
        // effects, and every candidate is expected to be a Core build.
        match unsafe { Library::new(&path) }.and_then(|lib| unsafe { Api::load(lib) }) {
            Ok(api) => return Ok(api),
            Err(e) => tried.push(format!("  {}: {e}", Path::new(&path).display())),
        }
    }
    Err(tried.join("\n"))
}

fn api() -> Result<&'static Api, CoreError> {
    static API: OnceLock<Result<Api, String>> = OnceLock::new();
    API.get_or_init(load)
        .as_ref()
        .map_err(|tried| CoreError::Unavailable(tried.clone()))
}

/// `csmGetVersion()` as `(major, minor, patch)`.
pub fn core_version() -> Result<(u32, u32, u32), CoreError> {
    let api = api()?;
    // SAFETY: no arguments, returns a plain integer.
    let v = unsafe { (api.csmGetVersion)() };
    Ok((v >> 24, (v >> 16) & 0xFF, v & 0xFFFF))
}

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error(
        "Live2D Cubism Core could not be loaded. Download the Cubism SDK for Native, accept its \
         terms, and set SSE_CUBISM_CORE to the Core shared library (or SSE_CUBISM_CORE_DIR to \
         the SDK's Core directory, or place the library next to the sse executable). Tried:\n{0}"
    )]
    Unavailable(String),
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
    api: &'static Api,
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
        let api = api()?;
        let mut mem = Aligned::new(bytes.len(), 64);
        mem.slice_mut().copy_from_slice(bytes);
        let size = bytes.len() as c_uint;
        // SAFETY: `mem` is 64-byte aligned, `size` bytes long and owned by the returned Moc.
        let (moc_version, ptr) = unsafe {
            let v = (api.csmGetMocVersion)(mem.as_mut_ptr(), size);
            (v, (api.csmReviveMocInPlace)(mem.as_mut_ptr(), size))
        };
        if ptr.is_null() {
            return Err(CoreError::InvalidMoc);
        }
        Ok(Arc::new(Self {
            api,
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
    api: &'static Api,
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
        let api = moc.api;
        // SAFETY: `moc.ptr` is a revived moc kept alive by the Arc held in the Model.
        let size = unsafe { (api.csmGetSizeofModel)(moc.ptr) } as usize;
        let mut mem = Aligned::new(size, 16);
        // SAFETY: `mem` is 16-byte aligned and `size` bytes long.
        let ptr =
            unsafe { (api.csmInitializeModelInPlace)(moc.ptr, mem.as_mut_ptr(), size as c_uint) };
        if ptr.is_null() {
            return Err(CoreError::InitFailed);
        }
        // SAFETY: all getters below read arrays sized by the matching count, owned by `mem`.
        unsafe {
            let np = (api.csmGetParameterCount)(ptr) as usize;
            let nparts = (api.csmGetPartCount)(ptr) as usize;
            let nd = (api.csmGetDrawableCount)(ptr) as usize;
            let mut canvas = Canvas {
                size_px: [0.0; 2],
                origin_px: [0.0; 2],
                pixels_per_unit: 0.0,
            };
            let mut s = CsmVector2 { x: 0.0, y: 0.0 };
            let mut o = CsmVector2 { x: 0.0, y: 0.0 };
            (api.csmReadCanvasInfo)(ptr, &mut s, &mut o, &mut canvas.pixels_per_unit);
            canvas.size_px = [s.x, s.y];
            canvas.origin_px = [o.x, o.y];

            let cflags = std::slice::from_raw_parts((api.csmGetDrawableConstantFlags)(ptr), nd);
            let tex = std::slice::from_raw_parts((api.csmGetDrawableTextureIndices)(ptr), nd);
            let mask_counts = std::slice::from_raw_parts((api.csmGetDrawableMaskCounts)(ptr), nd);
            let masks = (api.csmGetDrawableMasks)(ptr);
            let vcounts = std::slice::from_raw_parts((api.csmGetDrawableVertexCounts)(ptr), nd);
            let uvs = (api.csmGetDrawableVertexUvs)(ptr);
            let icounts = std::slice::from_raw_parts((api.csmGetDrawableIndexCounts)(ptr), nd);
            let indices = (api.csmGetDrawableIndices)(ptr);
            let dids = ids((api.csmGetDrawableIds)(ptr), nd);
            let drawables = (0..nd)
                .map(|i| {
                    let m = std::slice::from_raw_parts(*masks.add(i), mask_counts[i] as usize);
                    let uv = std::slice::from_raw_parts(*uvs.add(i), vcounts[i] as usize);
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
                parameter_ids: ids((api.csmGetParameterIds)(ptr), np),
                parameter_min: floats((api.csmGetParameterMinimumValues)(ptr), np),
                parameter_max: floats((api.csmGetParameterMaximumValues)(ptr), np),
                parameter_default: floats((api.csmGetParameterDefaultValues)(ptr), np),
                part_ids: ids((api.csmGetPartIds)(ptr), nparts),
                drawables,
                canvas,
                api,
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
            let dst = std::slice::from_raw_parts_mut((self.api.csmGetParameterValues)(self.ptr), n);
            dst.copy_from_slice(&params[..n]);
            if let Some(parts) = part_opacities {
                let np = self.part_ids.len();
                let dst =
                    std::slice::from_raw_parts_mut((self.api.csmGetPartOpacities)(self.ptr), np);
                dst.copy_from_slice(&parts[..np]);
            }
            (self.api.csmResetDrawableDynamicFlags)(self.ptr);
            (self.api.csmUpdateModel)(self.ptr);
        }
    }

    /// Copies out the drawable state after [`Model::update`].
    pub fn drawable_frames(&self) -> Vec<DrawableFrame> {
        let nd = self.drawables.len();
        // SAFETY: arrays sized by drawable count / vertex counts, valid until next update.
        unsafe {
            let dflags =
                std::slice::from_raw_parts((self.api.csmGetDrawableDynamicFlags)(self.ptr), nd);
            let op = std::slice::from_raw_parts((self.api.csmGetDrawableOpacities)(self.ptr), nd);
            let vcounts =
                std::slice::from_raw_parts((self.api.csmGetDrawableVertexCounts)(self.ptr), nd);
            let pos = (self.api.csmGetDrawableVertexPositions)(self.ptr);
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
        let orders =
            unsafe { std::slice::from_raw_parts((self.api.csmGetRenderOrders)(self.ptr), nd) };
        let mut idx: Vec<usize> = (0..nd).collect();
        idx.sort_by_key(|&i| orders[i]);
        idx
    }
}
