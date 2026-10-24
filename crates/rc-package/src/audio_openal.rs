//! Windows/Android OpenAL Soft loopback backend. Explicit library, no speaker device.
//! Unsafe boundary: typed pinned-header C ABI, owned DLL and ALC lifetimes,
//! bounded buffers, thread confinement and scoped current-context restoration.
//!
//! ```compile_fail
//! fn needs_send<T: Send>() {}
//! needs_send::<rc_package::audio_openal::OpenAl>();
//! ```
use crate::audio_device::*;
use std::{
    collections::BTreeSet,
    ffi::{c_char, c_void, CStr},
    marker::PhantomData,
    path::{Path, PathBuf},
    rc::Rc,
    thread::{self, ThreadId},
};
type Ptr = *mut c_void;
type SetThread = unsafe extern "C" fn(Ptr) -> i8;
type GetThread = unsafe extern "C" fn() -> Ptr;
#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn LoadLibraryExW(path: *const u16, file: Ptr, flags: u32) -> Ptr;
    fn GetProcAddress(module: Ptr, name: *const u8) -> Ptr;
    fn FreeLibrary(module: Ptr) -> i32;
}
#[cfg(target_os = "android")]
#[link(name = "dl")]
unsafe extern "C" {
    fn dlopen(path: *const c_char, flags: i32) -> Ptr;
    fn dlsym(library: Ptr, name: *const c_char) -> Ptr;
    fn dlclose(library: Ptr) -> i32;
    fn dlerror() -> *const c_char;
}
struct Library(Ptr);
impl Library {
    #[cfg(windows)]
    fn load(path: &Path) -> Result<Self, String> {
        use std::os::windows::ffi::OsStrExt;
        let path = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
        let wide = path
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        // Only the explicitly resolved DLL's directory and System32 supply imports.
        let p = unsafe { LoadLibraryExW(wide.as_ptr(), std::ptr::null_mut(), 0x100 | 0x800) };
        if p.is_null() {
            return Err(format!(
                "Load OpenAL failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(Self(p))
    }
    #[cfg(windows)]
    fn symbol(&self, name: &[u8]) -> Result<Ptr, String> {
        let p = unsafe { GetProcAddress(self.0, name.as_ptr()) };
        if p.is_null() {
            return Err(format!(
                "Missing OpenAL export {}",
                String::from_utf8_lossy(name)
            ));
        }
        Ok(p)
    }
    #[cfg(target_os = "android")]
    fn load(path: &Path) -> Result<Self, String> {
        use std::os::unix::ffi::OsStrExt;
        let absolute = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
        let name =
            std::ffi::CString::new(absolute.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
        // RTLD_NOW | RTLD_LOCAL. The APK extracts this library into its own nativeLibraryDir.
        let p = unsafe { dlopen(name.as_ptr(), 2) };
        if p.is_null() {
            return Err(Self::error("dlopen"));
        }
        Ok(Self(p))
    }
    #[cfg(target_os = "android")]
    fn error(operation: &str) -> String {
        let p = unsafe { dlerror() };
        if p.is_null() {
            operation.into()
        } else {
            format!(
                "{operation}: {}",
                unsafe { CStr::from_ptr(p) }.to_string_lossy()
            )
        }
    }
    #[cfg(target_os = "android")]
    fn symbol(&self, name: &[u8]) -> Result<Ptr, String> {
        let p = unsafe { dlsym(self.0, name.as_ptr().cast()) };
        if p.is_null() {
            Err(Self::error("dlsym"))
        } else {
            Ok(p)
        }
    }
}
impl Drop for Library {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            FreeLibrary(self.0);
        }
        #[cfg(target_os = "android")]
        unsafe {
            dlclose(self.0);
        }
    }
}
macro_rules! api {
    ($($field:ident:$ty:ty=$symbol:literal),*$(,)?)=>{
        struct Api {$($field:$ty,)*thread_set:SetThread,thread_get:GetThread,_library:Library}
        impl Api {fn load(path:&Path)->Result<Self,String>{let library=Library::load(path)?;
            let get:unsafe extern "C" fn(Ptr,*const c_char)->Ptr=unsafe{std::mem::transmute::<Ptr,unsafe extern "C" fn(Ptr,*const c_char)->Ptr>(library.symbol(b"alcGetProcAddress\0")?)};
            let set=unsafe{get(std::ptr::null_mut(),c"alcSetThreadContext".as_ptr())};let current=unsafe{get(std::ptr::null_mut(),c"alcGetThreadContext".as_ptr())};
            if set.is_null()||current.is_null(){return Err("OpenAL driver lacks ALC_EXT_thread_local_context".into());}
            Ok(Self {$($field:unsafe{std::mem::transmute::<Ptr,$ty>(library.symbol(concat!($symbol,"\0").as_bytes())?)},)*thread_set:unsafe{std::mem::transmute::<Ptr,SetThread>(set)},thread_get:unsafe{std::mem::transmute::<Ptr,GetThread>(current)},_library:library})}}
    }
}
api! {
    listener3:unsafe extern "C" fn(i32,f32,f32,f32)="alListener3f",
    listenerfv:unsafe extern "C" fn(i32,*const f32)="alListenerfv",
    proc:unsafe extern "C" fn(Ptr,*const c_char)->Ptr="alcGetProcAddress",
    extension:unsafe extern "C" fn(Ptr,*const c_char)->i8="alcIsExtensionPresent",
    create_context:unsafe extern "C" fn(Ptr,*const i32)->Ptr="alcCreateContext",
    destroy_context:unsafe extern "C" fn(Ptr)="alcDestroyContext",
    close_device:unsafe extern "C" fn(Ptr)->i8="alcCloseDevice",
    alc_error:unsafe extern "C" fn(Ptr)->i32="alcGetError",
    process:unsafe extern "C" fn(Ptr)="alcProcessContext",
    suspend:unsafe extern "C" fn(Ptr)="alcSuspendContext",
    error:unsafe extern "C" fn()->i32="alGetError",
    string:unsafe extern "C" fn(i32)->*const c_char="alGetString",
    gen_sources:unsafe extern "C" fn(i32,*mut u32)="alGenSources",
    delete_sources:unsafe extern "C" fn(i32,*const u32)="alDeleteSources",
    gen_buffers:unsafe extern "C" fn(i32,*mut u32)="alGenBuffers",
    delete_buffers:unsafe extern "C" fn(i32,*const u32)="alDeleteBuffers",
    data:unsafe extern "C" fn(u32,i32,*const c_void,i32,i32)="alBufferData",
    float:unsafe extern "C" fn(u32,i32,f32)="alSourcef",
    integer:unsafe extern "C" fn(u32,i32,i32)="alSourcei",
    vector:unsafe extern "C" fn(u32,i32,f32,f32,f32)="alSource3f",
    play:unsafe extern "C" fn(u32)="alSourcePlay",
    pause:unsafe extern "C" fn(u32)="alSourcePause",
    stop:unsafe extern "C" fn(u32)="alSourceStop",
    get:unsafe extern "C" fn(u32,i32,*mut i32)="alGetSourcei",
    queue:unsafe extern "C" fn(u32,i32,*const u32)="alSourceQueueBuffers",
    unqueue:unsafe extern "C" fn(u32,i32,*mut u32)="alSourceUnqueueBuffers",
}
type LoopOpen = unsafe extern "C" fn(*const c_char) -> Ptr;
type Render = unsafe extern "C" fn(Ptr, *mut c_void, i32);
type Supported = unsafe extern "C" fn(Ptr, i32, i32, i32) -> i8;
struct Current<'a> {
    api: &'a Api,
    previous: Ptr,
}
impl<'a> Current<'a> {
    fn enter(api: &'a Api, context: Ptr) -> Result<Self, String> {
        if context.is_null() {
            return Err("OpenAL context is closed".into());
        }
        let previous = unsafe { (api.thread_get)() };
        if unsafe { (api.thread_set)(context) } == 0 {
            return Err("Cannot bind OpenAL context".into());
        }
        Ok(Self { api, previous })
    }
}
impl Drop for Current<'_> {
    fn drop(&mut self) {
        unsafe {
            (self.api.thread_set)(self.previous);
        }
    }
}
#[derive(Default, Clone, Debug)]
pub struct Stats {
    pub commands: usize,
    pub uploads: usize,
    pub upload_bytes: usize,
    pub renders: usize,
    pub frames: usize,
    pub resets: usize,
    pub unqueues: usize,
}
pub struct OpenAl {
    api: Api,
    open: LoopOpen,
    render: Render,
    supported: Supported,
    device: Ptr,
    context: Ptr,
    frequency: u32,
    thread: ThreadId,
    _thread_bound: PhantomData<Rc<()>>,
    sources: BTreeSet<u32>,
    buffers: BTreeSet<u32>,
    context_tag: u32,
    context_issued: bool,
    path: PathBuf,
    version: String,
    stats: Stats,
}
impl OpenAl {
    pub fn open_loopback(path: &Path, frequency: u32) -> Result<Self, String> {
        if !(8000..=192000).contains(&frequency) {
            return Err("Loopback rate must be 8000..192000".into());
        }
        let api = Api::load(path)?;
        if unsafe { (api.extension)(std::ptr::null_mut(), c"ALC_SOFT_loopback".as_ptr()) } == 0 {
            return Err("Loaded DLL lacks ALC_SOFT_loopback".into());
        }
        if unsafe {
            (api.extension)(
                std::ptr::null_mut(),
                c"ALC_EXT_thread_local_context".as_ptr(),
            )
        } == 0
        {
            return Err("Loaded DLL lacks thread-local contexts".into());
        }
        let address = |name: &CStr| -> Result<Ptr, String> {
            let p = unsafe { (api.proc)(std::ptr::null_mut(), name.as_ptr()) };
            if p.is_null() {
                return Err(format!(
                    "Missing loopback procedure {}",
                    name.to_string_lossy()
                ));
            }
            Ok(p)
        };
        let open =
            unsafe { std::mem::transmute::<Ptr, LoopOpen>(address(c"alcLoopbackOpenDeviceSOFT")?) };
        let render =
            unsafe { std::mem::transmute::<Ptr, Render>(address(c"alcRenderSamplesSOFT")?) };
        let supported = unsafe {
            std::mem::transmute::<Ptr, Supported>(address(c"alcIsRenderFormatSupportedSOFT")?)
        };
        let mut s = Self {
            api,
            open,
            render,
            supported,
            device: std::ptr::null_mut(),
            context: std::ptr::null_mut(),
            frequency,
            thread: thread::current().id(),
            _thread_bound: PhantomData,
            sources: BTreeSet::new(),
            buffers: BTreeSet::new(),
            context_tag: 0,
            context_issued: false,
            path: std::fs::canonicalize(path).map_err(|e| e.to_string())?,
            version: String::new(),
            stats: Stats::default(),
        };
        s.open_context()?;
        Ok(s)
    }
    fn owner_thread(&self) -> Result<(), String> {
        if thread::current().id() != self.thread {
            return Err("OpenAL backend used from a foreign thread".into());
        }
        Ok(())
    }
    fn check(&self, label: &str) -> Result<(), String> {
        let error = unsafe { (self.api.error)() };
        if error != 0 {
            return Err(format!("OpenAL {label}: 0x{error:x}"));
        }
        Ok(())
    }
    fn open_context(&mut self) -> Result<(), String> {
        self.device = unsafe { (self.open)(std::ptr::null()) };
        if self.device.is_null() {
            return Err("Cannot open OpenAL loopback device".into());
        }
        if unsafe { (self.supported)(self.device, self.frequency as i32, 0x1501, 0x1402) } == 0 {
            return Err("Stereo S16 loopback format unsupported".into());
        }
        let attrs = [
            0x1990,
            0x1501,
            0x1991,
            0x1402,
            0x1007,
            self.frequency as i32,
            0,
        ];
        self.context = unsafe { (self.api.create_context)(self.device, attrs.as_ptr()) };
        if self.context.is_null() {
            return Err(format!("Cannot create loopback context: 0x{:x}", unsafe {
                (self.api.alc_error)(self.device)
            }));
        }
        self.context_tag = self
            .context_tag
            .checked_add(1)
            .ok_or("OpenAL context tags exhausted")?;
        self.context_issued = false;
        let _current = Current::enter(&self.api, self.context)?;
        let p = unsafe { (self.api.string)(0xb002) };
        if p.is_null() {
            return Err("Missing OpenAL version string".into());
        }
        self.version = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
        self.check("version")
    }
    fn destroy(&mut self) -> Result<(), String> {
        self.owner_thread()?;
        let mut error = None;
        if !self.context.is_null() {
            let previous = unsafe { (self.api.thread_get)() };
            if unsafe { (self.api.thread_set)(self.context) } != 0 {
                for &s in &self.sources {
                    unsafe {
                        (self.api.stop)(s);
                    }
                }
                let sources = self.sources.iter().copied().collect::<Vec<_>>();
                let buffers = self.buffers.iter().copied().collect::<Vec<_>>();
                if !sources.is_empty() {
                    unsafe {
                        (self.api.delete_sources)(sources.len() as i32, sources.as_ptr());
                    }
                }
                if !buffers.is_empty() {
                    unsafe {
                        (self.api.delete_buffers)(buffers.len() as i32, buffers.as_ptr());
                    }
                }
                error = self.check("cleanup").err();
            } else {
                error = Some("Cannot bind context for cleanup".into());
            }
            unsafe {
                (self.api.thread_set)(if previous == self.context {
                    std::ptr::null_mut()
                } else {
                    previous
                });
                (self.api.destroy_context)(self.context);
            }
            self.context = std::ptr::null_mut();
            self.sources.clear();
            self.buffers.clear();
            self.context_issued = false;
        }
        if !self.device.is_null() {
            if unsafe { (self.api.close_device)(self.device) } == 0 {
                return Err("Cannot close OpenAL loopback device".into());
            }
            self.device = std::ptr::null_mut();
        }
        if let Some(e) = error {
            return Err(e);
        }
        Ok(())
    }
    pub fn version(&self) -> &str {
        &self.version
    }
    pub fn library_path(&self) -> &Path {
        &self.path
    }
    pub fn stats(&self) -> &Stats {
        &self.stats
    }
    pub fn live_objects(&self) -> (usize, usize) {
        (self.sources.len(), self.buffers.len())
    }
}
impl Backend for OpenAl {
    fn listener(
        &mut self,
        position: [f32; 3],
        forward: [f32; 3],
        up: [f32; 3],
    ) -> Result<(), String> {
        self.owner_thread()?;
        let _current = Current::enter(&self.api, self.context)?;
        let orientation = [forward[0], forward[1], forward[2], up[0], up[1], up[2]];
        unsafe {
            (self.api.listener3)(0x1004, position[0], position[1], position[2]);
            (self.api.listenerfv)(0x100f, orientation.as_ptr());
        }
        self.check("listener")
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            eax: false,
            context: true,
            music: false,
        }
    }
    fn reset(&mut self) -> Result<(), String> {
        self.owner_thread()?;
        self.destroy()?;
        self.open_context()?;
        self.stats.resets += 1;
        Ok(())
    }
    fn create(&mut self, kind: Kind) -> Result<u32, String> {
        self.owner_thread()?;
        let _current = Current::enter(&self.api, self.context)?;
        if kind == Kind::Context {
            if self.context_issued {
                return Err("Backend owns exactly one ALC context".into());
            }
            self.context_issued = true;
            return Ok(self.context_tag);
        }
        let mut id = 0;
        unsafe {
            match kind {
                Kind::Source => (self.api.gen_sources)(1, &mut id),
                Kind::Buffer => (self.api.gen_buffers)(1, &mut id),
                Kind::Context => unreachable!(),
            }
        }
        if id != 0 {
            if kind == Kind::Source {
                self.sources.insert(id);
            } else {
                self.buffers.insert(id);
            }
        }
        self.check("generate")?;
        if id == 0 {
            return Err("OpenAL returned null object name".into());
        }
        Ok(id)
    }
    fn execute(&mut self, op: &Op) -> Result<Reply, String> {
        self.owner_thread()?;
        let _current = Current::enter(&self.api, self.context)?;
        self.stats.commands += 1;
        let mut reply = Reply::Unit;
        // API validates object names/parameters; owned slices and counts remain
        // alive throughout each C call and never contain unchecked pointers.
        unsafe {
            match op {
                Op::Upload {
                    buffer,
                    format,
                    frequency,
                    bytes,
                } => {
                    if bytes.len() > crate::audio_stream_data::MAX_BYTES || *frequency > 1_000_000 {
                        return Err("OpenAL upload limit exceeded".into());
                    }
                    (self.api.data)(
                        *buffer,
                        *format as i32,
                        bytes.as_ptr().cast(),
                        i32::try_from(bytes.len()).map_err(|_| "Upload too large")?,
                        *frequency as i32,
                    );
                    self.stats.uploads += 1;
                    self.stats.upload_bytes += bytes.len();
                }
                Op::Float {
                    source,
                    parameter,
                    value,
                } => (self.api.float)(*source, *parameter as i32, f32::from_bits(*value)),
                Op::Integer {
                    source,
                    parameter,
                    value,
                } => (self.api.integer)(*source, *parameter as i32, *value as i32),
                Op::Vector {
                    source,
                    parameter,
                    value,
                } => (self.api.vector)(
                    *source,
                    *parameter as i32,
                    f32::from_bits(value[0]),
                    f32::from_bits(value[1]),
                    f32::from_bits(value[2]),
                ),
                Op::Play(s) => (self.api.play)(*s),
                Op::Pause(s) => (self.api.pause)(*s),
                Op::Stop(s) => (self.api.stop)(*s),
                Op::Queue { source, buffers } => {
                    if buffers.len() > 4096 {
                        return Err("AL queue too large".into());
                    }
                    (self.api.queue)(*source, buffers.len() as i32, buffers.as_ptr());
                }
                Op::Unqueue {
                    source, expected, ..
                } => {
                    if expected.len() > 4096 {
                        return Err("AL unqueue too large".into());
                    }
                    let mut ids = vec![0; expected.len()];
                    (self.api.unqueue)(*source, ids.len() as i32, ids.as_mut_ptr());
                    reply = Reply::Buffers(ids);
                    self.stats.unqueues += 1;
                }
                Op::Query {
                    source, parameter, ..
                } => {
                    let mut value = 0;
                    (self.api.get)(*source, *parameter as i32, &mut value);
                    reply = Reply::Scalar(value);
                }
                Op::DeleteBuffer(id) => {
                    if !self.buffers.contains(id) {
                        return Err("Unknown owned AL buffer delete".into());
                    }
                    (self.api.delete_buffers)(1, id);
                    self.check("delete buffer")?;
                    self.buffers.remove(id);
                }
                Op::Process(tag) | Op::Suspend(tag) => {
                    if !self.context_issued || *tag != self.context_tag {
                        return Err("Unknown ALC context token".into());
                    }
                    if matches!(op, Op::Process(_)) {
                        (self.api.process)(self.context);
                    } else {
                        (self.api.suspend)(self.context);
                    }
                    let e = (self.api.alc_error)(self.device);
                    if e != 0 {
                        return Err(format!("ALC context operation: 0x{e:x}"));
                    }
                }
                Op::ClearError => {
                    (self.api.error)();
                }
                Op::Check(label) => {
                    self.check(label)?;
                }
                Op::Eax { .. } | Op::MusicPause(_) | Op::MusicStop => {
                    return Err("Unsupported EAX/music operation on loopback backend".into())
                }
            }
        }
        self.check("execute")?;
        Ok(reply)
    }
    fn supports_render(&self) -> bool {
        true
    }
    fn render(&mut self, frames: usize) -> Result<Rendered, String> {
        self.owner_thread()?;
        if !(1..=1_000_000).contains(&frames) {
            return Err("Loopback frame limit exceeded".into());
        }
        let _current = Current::enter(&self.api, self.context)?;
        let mut samples = vec![0i16; frames * 2];
        unsafe {
            (self.api.process)(self.context);
            (self.render)(self.device, samples.as_mut_ptr().cast(), frames as i32);
            (self.api.suspend)(self.context);
        }
        let error = unsafe { (self.api.alc_error)(self.device) };
        if error != 0 {
            return Err(format!("Loopback render: 0x{error:x}"));
        }
        self.check("render")?;
        self.stats.renders += 1;
        self.stats.frames += frames;
        Ok(Rendered {
            frequency: self.frequency,
            samples,
        })
    }
}
impl Drop for OpenAl {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}
