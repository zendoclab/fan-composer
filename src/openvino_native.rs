//! Minimal OpenVINO 2026 C ABI bridge. Handles stay on the inference thread.
//! Signatures and F32=4 verified against the installed openvino/c headers.
use libloading::Library;
use std::{
    ffi::{CStr, CString, c_char, c_void},
    path::{Path, PathBuf},
    ptr,
};
type Handle = *mut c_void;
type Status = i32;
type Free = unsafe extern "C" fn(Handle);
#[repr(C)]
struct Shape {
    rank: i64,
    dims: *mut i64,
}
#[repr(C)]
struct Devices {
    devices: *mut *mut c_char,
    size: usize,
}
struct Api {
    create: unsafe extern "C" fn(*mut Handle) -> Status,
    read: unsafe extern "C" fn(Handle, *const c_char, *const c_char, *mut Handle) -> Status,
    compile: unsafe extern "C" fn(Handle, Handle, *const c_char, usize, *mut Handle, ...) -> Status,
    devices: unsafe extern "C" fn(Handle, *mut Devices) -> Status,
    devices_free: unsafe extern "C" fn(*mut Devices),
    property: unsafe extern "C" fn(Handle, *const c_char, *mut *mut c_char) -> Status,
    string_free: unsafe extern "C" fn(*const c_char),
    request: unsafe extern "C" fn(Handle, *mut Handle) -> Status,
    tensor: unsafe extern "C" fn(i32, Shape, *mut Handle) -> Status,
    tensor_data: unsafe extern "C" fn(Handle, *mut Handle) -> Status,
    tensor_size: unsafe extern "C" fn(Handle, *mut usize) -> Status,
    tensor_type: unsafe extern "C" fn(Handle, *mut i32) -> Status,
    set_input: unsafe extern "C" fn(Handle, Handle) -> Status,
    get_output: unsafe extern "C" fn(Handle, *mut Handle) -> Status,
    infer: unsafe extern "C" fn(Handle) -> Status,
    core_free: Free,
    model_free: Free,
    compiled_free: Free,
    request_free: Free,
    tensor_free: Free,
    last_error: unsafe extern "C" fn() -> *const c_char,
    _library: Library,
}
impl Api {
    fn load(path: &Path) -> Result<Self, String> {
        let canonical = path
            .canonicalize()
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let path = canonical.as_path();
        // Load dependencies beside the chosen DLL without changing process/global PATH.
        #[cfg(windows)]
        let library: Library = unsafe {
            libloading::os::windows::Library::load_with_flags(path, 0x00000100 | 0x00001000)
                .map_err(|e| format!("{e:?}; {}", path.display()))?
                .into()
        };
        #[cfg(not(windows))]
        let library = unsafe { Library::new(path).map_err(|e| e.to_string())? };
        macro_rules! symbol {
            ($name:literal) => {
                unsafe {
                    *library
                        .get(concat!($name, "\0").as_bytes())
                        .map_err(|e| e.to_string())?
                }
            };
        }
        Ok(Self {
            create: symbol!("ov_core_create"),
            read: symbol!("ov_core_read_model"),
            compile: symbol!("ov_core_compile_model"),
            devices: symbol!("ov_core_get_available_devices"),
            devices_free: symbol!("ov_available_devices_free"),
            property: symbol!("ov_compiled_model_get_property"),
            string_free: symbol!("ov_free"),
            request: symbol!("ov_compiled_model_create_infer_request"),
            tensor: symbol!("ov_tensor_create"),
            tensor_data: symbol!("ov_tensor_data"),
            tensor_size: symbol!("ov_tensor_get_size"),
            tensor_type: symbol!("ov_tensor_get_element_type"),
            set_input: symbol!("ov_infer_request_set_input_tensor"),
            get_output: symbol!("ov_infer_request_get_output_tensor"),
            infer: symbol!("ov_infer_request_infer"),
            core_free: symbol!("ov_core_free"),
            model_free: symbol!("ov_model_free"),
            compiled_free: symbol!("ov_compiled_model_free"),
            request_free: symbol!("ov_infer_request_free"),
            tensor_free: symbol!("ov_tensor_free"),
            last_error: symbol!("ov_get_last_err_msg"),
            _library: library,
        })
    }
    fn check(&self, status: Status, operation: &str) -> Result<(), String> {
        if status == 0 {
            return Ok(());
        }
        let message = unsafe {
            let p = (self.last_error)();
            if p.is_null() {
                "No runtime detail".into()
            } else {
                CStr::from_ptr(p).to_string_lossy().into_owned()
            }
        };
        Err(format!("{operation}: OpenVINO status {status}: {message}"))
    }
}
pub struct Session {
    api: Api,
    core: Handle,
    model: Handle,
    compiled: Handle,
    request: Handle,
    input: Handle,
    pub execution_device: String,
    pub available_devices: Vec<String>,
    pub compile_ms: f64,
}
impl Session {
    pub fn load(runtime: &Path, model: &Path, device: &str) -> Result<Self, String> {
        let api = Api::load(runtime)?;
        let mut s = Self {
            api,
            core: ptr::null_mut(),
            model: ptr::null_mut(),
            compiled: ptr::null_mut(),
            request: ptr::null_mut(),
            input: ptr::null_mut(),
            execution_device: String::new(),
            available_devices: vec![],
            compile_ms: 0.0,
        };
        // All handles are owned here; partial initialization is cleaned up by Drop.
        unsafe {
            s.api.check((s.api.create)(&mut s.core), "create core")?;
            let mut devices = Devices {
                devices: ptr::null_mut(),
                size: 0,
            };
            s.api
                .check((s.api.devices)(s.core, &mut devices), "enumerate devices")?;
            if devices.size <= 64 && !devices.devices.is_null() {
                for i in 0..devices.size {
                    let p = *devices.devices.add(i);
                    if !p.is_null() {
                        s.available_devices
                            .push(CStr::from_ptr(p).to_string_lossy().into_owned());
                    }
                }
            }
            (s.api.devices_free)(&mut devices);
            if !s
                .available_devices
                .iter()
                .any(|v| v == device || v.starts_with(&format!("{device}.")))
            {
                return Err(format!(
                    "{device} not available; detected {:?}",
                    s.available_devices
                ));
            }
            let model_path =
                CString::new(model.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
            s.api.check(
                (s.api.read)(s.core, model_path.as_ptr(), ptr::null(), &mut s.model),
                "read policy IR",
            )?;
            let device_name = CString::new(device).map_err(|e| e.to_string())?;
            let began = std::time::Instant::now();
            s.api.check(
                (s.api.compile)(s.core, s.model, device_name.as_ptr(), 0, &mut s.compiled),
                "compile policy",
            )?;
            s.compile_ms = began.elapsed().as_secs_f64() * 1000.0;
            let mut actual = ptr::null_mut();
            s.api.check(
                (s.api.property)(s.compiled, c"EXECUTION_DEVICES".as_ptr(), &mut actual),
                "execution device",
            )?;
            if actual.is_null() {
                return Err("Empty EXECUTION_DEVICES".into());
            }
            s.execution_device = CStr::from_ptr(actual).to_string_lossy().into_owned();
            (s.api.string_free)(actual);
            if !s.execution_device.contains(device) {
                return Err(format!("Requested {device}, actual {}", s.execution_device));
            }
            s.api.check(
                (s.api.request)(s.compiled, &mut s.request),
                "create infer request",
            )?;
            let mut dims = [1_i64, 16];
            s.api.check(
                (s.api.tensor)(
                    4,
                    Shape {
                        rank: 2,
                        dims: dims.as_mut_ptr(),
                    },
                    &mut s.input,
                ),
                "create F32 input",
            )?;
            s.api
                .check((s.api.set_input)(s.request, s.input), "bind input")?;
        }
        s.infer(&[0.5; 16])?; // A successful compile alone never marks the backend ready.
        Ok(s)
    }
    pub fn infer(&mut self, input: &[f32; 16]) -> Result<[f32; 6], String> {
        if input.iter().any(|v| !v.is_finite()) {
            return Err("Nonfinite music feature".into());
        }
        unsafe {
            let mut data = ptr::null_mut();
            self.api
                .check((self.api.tensor_data)(self.input, &mut data), "input data")?;
            if data.is_null() {
                return Err("Null input data".into());
            }
            // Owned F32 [1,16] tensor, accessed only by this thread.
            ptr::copy_nonoverlapping(input.as_ptr(), data.cast::<f32>(), 16);
            self.api
                .check((self.api.infer)(self.request), "policy inference")?;
            let mut output = ptr::null_mut();
            self.api.check(
                (self.api.get_output)(self.request, &mut output),
                "output tensor",
            )?;
            struct Tensor {
                pointer: Handle,
                free: Free,
            }
            impl Drop for Tensor {
                fn drop(&mut self) {
                    unsafe {
                        (self.free)(self.pointer);
                    }
                }
            }
            let _output = Tensor {
                pointer: output,
                free: self.api.tensor_free,
            };
            let mut count = 0;
            let mut kind = 0;
            self.api
                .check((self.api.tensor_size)(output, &mut count), "output size")?;
            self.api
                .check((self.api.tensor_type)(output, &mut kind), "output type")?;
            if count != 6 || kind != 4 {
                return Err(format!(
                    "Expected F32 [1,6], got type {kind}, {count} elements"
                ));
            }
            let mut data = ptr::null_mut();
            self.api
                .check((self.api.tensor_data)(output, &mut data), "output data")?;
            if data.is_null() {
                return Err("Null policy output".into());
            }
            let mut result = [0.0; 6];
            result.copy_from_slice(std::slice::from_raw_parts(data.cast::<f32>(), 6));
            if result.iter().any(|v| !v.is_finite()) {
                return Err("Nonfinite NPU/CPU policy output".into());
            }
            Ok(result.map(|v| v.clamp(0.0, 1.0)))
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        unsafe {
            for (p, free) in [
                (self.request, self.api.request_free),
                (self.input, self.api.tensor_free),
                (self.compiled, self.api.compiled_free),
                (self.model, self.api.model_free),
                (self.core, self.api.core_free),
            ] {
                if !p.is_null() {
                    free(p);
                }
            }
        }
    }
}
pub fn asset_paths() -> (PathBuf, PathBuf) {
    let exe = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    let development = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let model = if exe.join("models/policy.xml").exists() {
        exe.join("models/policy.xml")
    } else {
        development.join("models/policy.xml")
    };
    let runtime = if exe.join("runtime/openvino_c.dll").exists() {
        exe.join("runtime/openvino_c.dll")
    } else {
        development.join(".venv/Lib/site-packages/openvino/libs/openvino_c.dll")
    };
    (runtime, model)
}
