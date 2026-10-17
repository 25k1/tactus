use serde_json::Value;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr::NonNull;
use std::sync::Arc;
use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LOAD_WITH_ALTERED_SEARCH_PATH, LoadLibraryExW};

type Handle = *mut c_void;
type Procedure = unsafe extern "system" fn() -> isize;
type Create = unsafe extern "C" fn(*const c_char) -> Handle;
type Release = unsafe extern "C" fn(Handle);
type Lookup = unsafe extern "C" fn(Handle, *const c_char) -> c_int;
type Recognize = unsafe extern "C" fn(Handle, f32) -> Handle;
type RecognizeGrammar = unsafe extern "C" fn(Handle, f32, *const c_char) -> Handle;
type Accept = unsafe extern "C" fn(Handle, *const i16, c_int) -> c_int;
type Read = unsafe extern "C" fn(Handle) -> *const c_char;
type Level = unsafe extern "C" fn(c_int);

struct Functions {
    model_new: Create,
    model_free: Release,
    find_word: Lookup,
    recognizer_new: Recognize,
    recognizer_new_grm: RecognizeGrammar,
    accept: Accept,
    result: Read,
    final_result: Read,
    reset: Release,
    recognizer_free: Release,
    set_log_level: Level,
}

pub struct Vosk {
    functions: Functions,
}

unsafe impl Send for Vosk {}
unsafe impl Sync for Vosk {}

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
}

fn short(path: &Path) -> Result<CString, String> {
    let long = wide(path);
    let mut buffer = vec![0u16; 1024];
    let length = unsafe { GetShortPathNameW(long.as_ptr(), buffer.as_mut_ptr(), buffer.len() as u32) } as usize;
    let text = if length == 0 || length > buffer.len() {
        path.to_string_lossy().into_owned()
    } else {
        String::from_utf16_lossy(&buffer[..length])
    };
    CString::new(text).map_err(|error| error.to_string())
}

macro_rules! symbol {
    ($library:expr, $name:literal, $kind:ty) => {{
        let address = unsafe { GetProcAddress($library, concat!($name, "\0").as_ptr()) };
        let address = address.ok_or_else(|| format!("libvosk.dll has no {}", $name))?;
        unsafe { std::mem::transmute::<Procedure, $kind>(address) }
    }};
}

impl Vosk {
    pub fn load(folder: &Path) -> Result<Arc<Vosk>, String> {
        let path = wide(&folder.join("libvosk.dll"));
        let library = unsafe { LoadLibraryExW(path.as_ptr(), std::ptr::null_mut(), LOAD_WITH_ALTERED_SEARCH_PATH) };
        if library.is_null() {
            return Err(format!("could not load {}", folder.join("libvosk.dll").display()));
        }
        let functions = Functions {
            model_new: symbol!(library, "vosk_model_new", Create),
            model_free: symbol!(library, "vosk_model_free", Release),
            find_word: symbol!(library, "vosk_model_find_word", Lookup),
            recognizer_new: symbol!(library, "vosk_recognizer_new", Recognize),
            recognizer_new_grm: symbol!(library, "vosk_recognizer_new_grm", RecognizeGrammar),
            accept: symbol!(library, "vosk_recognizer_accept_waveform_s", Accept),
            result: symbol!(library, "vosk_recognizer_result", Read),
            final_result: symbol!(library, "vosk_recognizer_final_result", Read),
            reset: symbol!(library, "vosk_recognizer_reset", Release),
            recognizer_free: symbol!(library, "vosk_recognizer_free", Release),
            set_log_level: symbol!(library, "vosk_set_log_level", Level),
        };
        unsafe { (functions.set_log_level)(-1) };
        Ok(Arc::new(Vosk { functions }))
    }
}

pub struct Model {
    vosk: Arc<Vosk>,
    handle: NonNull<c_void>,
}

unsafe impl Send for Model {}
unsafe impl Sync for Model {}

impl Model {
    pub fn load(vosk: &Arc<Vosk>, path: &Path) -> Result<Arc<Model>, String> {
        let path = short(path)?;
        let handle = unsafe { (vosk.functions.model_new)(path.as_ptr()) };
        NonNull::new(handle)
            .map(|handle| Arc::new(Model { vosk: vosk.clone(), handle }))
            .ok_or_else(|| "the speech model could not be loaded".to_owned())
    }

    pub fn knows(&self, word: &str) -> bool {
        CString::new(word)
            .is_ok_and(|word| unsafe { (self.vosk.functions.find_word)(self.handle.as_ptr(), word.as_ptr()) } >= 0)
    }
}

impl Drop for Model {
    fn drop(&mut self) {
        unsafe { (self.vosk.functions.model_free)(self.handle.as_ptr()) };
    }
}

pub struct Recognizer {
    model: Arc<Model>,
    handle: NonNull<c_void>,
}

unsafe impl Send for Recognizer {}

fn text(json: *const c_char) -> String {
    if json.is_null() {
        return String::new();
    }
    let raw = unsafe { CStr::from_ptr(json) }.to_string_lossy();
    serde_json::from_str::<Value>(&raw)
        .ok()
        .and_then(|value| value.get("text").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_default()
}

impl Recognizer {
    pub fn new(model: &Arc<Model>, rate: u32, words: Option<&[&str]>) -> Result<Recognizer, String> {
        let functions = &model.vosk.functions;
        let handle = match words {
            None => unsafe { (functions.recognizer_new)(model.handle.as_ptr(), rate as f32) },
            Some(words) => {
                let grammar = Value::from(words.iter().copied().chain(["[unk]"]).collect::<Vec<_>>()).to_string();
                let grammar = CString::new(grammar).map_err(|error| error.to_string())?;
                unsafe { (functions.recognizer_new_grm)(model.handle.as_ptr(), rate as f32, grammar.as_ptr()) }
            }
        };
        NonNull::new(handle)
            .map(|handle| Recognizer { model: model.clone(), handle })
            .ok_or_else(|| "the recognizer could not be created".to_owned())
    }

    pub fn accept(&mut self, samples: &[i16]) -> bool {
        let length = c_int::try_from(samples.len()).unwrap_or(c_int::MAX);
        unsafe { (self.model.vosk.functions.accept)(self.handle.as_ptr(), samples.as_ptr(), length) == 1 }
    }

    pub fn result(&mut self) -> String {
        text(unsafe { (self.model.vosk.functions.result)(self.handle.as_ptr()) })
    }

    pub fn finish(&mut self) -> String {
        let heard = text(unsafe { (self.model.vosk.functions.final_result)(self.handle.as_ptr()) });
        self.reset();
        heard
    }

    pub fn reset(&mut self) {
        unsafe { (self.model.vosk.functions.reset)(self.handle.as_ptr()) };
    }
}

impl Drop for Recognizer {
    fn drop(&mut self) {
        unsafe { (self.model.vosk.functions.recognizer_free)(self.handle.as_ptr()) };
    }
}
