//! Slightly higher level helper for fontconfig
#![allow(clippy::mutex_atomic)]

use anyhow::{anyhow, ensure, Error};
pub use fontconfig::*;
use std::ffi::{CStr, CString};
use std::fmt;
use std::mem;
use std::os::raw::{c_char, c_int};
use std::ptr;
use std::sync::Mutex;

static FC_MONO: i32 = 100;
static FC_DUAL: i32 = 90;

lazy_static::lazy_static! {
    /// This is hideous and gross, but we don't have a lot of choice.
    /// The issue here is that the fontconfig library maintains some
    /// global state that is implicitly initialized by the various
    /// library functions.  There isn't a way for a single code path
    /// to maintain an isolated set of state.  In wezterm we only
    /// use fontconfig to discover files and then we don't need to
    /// talk to it again, so it is desirable to have it unload its
    /// various caches and references to fonts when we're done with
    /// it.  We use this counter to tell when we've released the
    /// final reference to a fontconfig object so that we can tell
    /// the library to shutdown.
    static ref NUM_OBJECTS :Mutex<usize> = Mutex::new(0);
}

fn add_object() {
    let mut num = NUM_OBJECTS.lock().unwrap();
    *num += 1;
    // log::trace!("fc object count + -> {}", *num);
}

fn release_object() {
    let mut num = NUM_OBJECTS.lock().unwrap();
    let count = *num - 1;
    *num = count;

    // log::trace!("fc object count - -> {}", *num);

    if count == 0 {
        // log::trace!("Finalize fontconfig!");
        // There are no more objects referencing the fontconfig
        // library, so we can release it now
        unsafe {
            FcFini();
        }
    }
}

pub struct FontSet {
    fonts: *mut FcFontSet,
}

impl Drop for FontSet {
    fn drop(&mut self) {
        unsafe {
            FcFontSetDestroy(self.fonts);
        }
        release_object();
    }
}

impl fmt::Debug for FontSet {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_list().entries(self.iter()).finish()
    }
}

pub struct FontSetIter<'a> {
    set: &'a FontSet,
    position: isize,
}

impl<'a> Iterator for FontSetIter<'a> {
    type Item = Pattern;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            if self.position == (*self.set.fonts).nfont as isize {
                None
            } else {
                let pat = *(*self.set.fonts)
                    .fonts
                    .offset(self.position)
                    .as_mut()
                    .unwrap();
                FcPatternReference(pat);
                add_object();
                self.position += 1;
                Some(Pattern { pat })
            }
        }
    }
}

impl FontSet {
    pub fn iter(&self) -> FontSetIter {
        FontSetIter {
            set: self,
            position: 0,
        }
    }
}

#[repr(C)]
pub enum MatchKind {
    Pattern = FcMatchPattern as isize,
}

pub struct FcResultWrap(FcResult);

impl FcResultWrap {
    pub fn succeeded(&self) -> bool {
        self.0 == FcResultMatch
    }

    pub fn as_err(&self) -> Error {
        // the compiler thinks we defined these globals, when all
        // we did was import them from elsewhere
        match self.0 {
            fontconfig::FcResultMatch => anyhow!("FcResultMatch"),
            fontconfig::FcResultNoMatch => anyhow!("FcResultNoMatch"),
            fontconfig::FcResultTypeMismatch => anyhow!("FcResultTypeMismatch"),
            fontconfig::FcResultNoId => anyhow!("FcResultNoId"),
            fontconfig::FcResultOutOfMemory => anyhow!("FcResultOutOfMemory"),
            _ => anyhow!("FcResult holds invalid value {}", self.0),
        }
    }

    pub fn result<T>(&self, t: T) -> Result<T, Error> {
        #[allow(non_upper_case_globals)]
        match self.0 {
            FcResultMatch => Ok(t),
            _ => Err(self.as_err()),
        }
    }
}

pub struct CharSet {
    cset: *mut FcCharSet,
}

impl Drop for CharSet {
    fn drop(&mut self) {
        unsafe {
            FcCharSetDestroy(self.cset);
        }
        release_object();
    }
}

impl CharSet {
    pub fn new() -> anyhow::Result<Self> {
        unsafe {
            let cset = FcCharSetCreate();
            ensure!(!cset.is_null(), "FcCharSetCreate failed");
            add_object();
            Ok(Self { cset })
        }
    }

    pub fn add(&mut self, c: char) -> anyhow::Result<()> {
        unsafe {
            ensure!(
                FcCharSetAddChar(self.cset, c as u32) != 0,
                "FcCharSetAddChar failed"
            );
            Ok(())
        }
    }
}

pub struct Pattern {
    pat: *mut FcPattern,
}

impl Pattern {
    pub fn new() -> Result<Pattern, Error> {
        unsafe {
            let p = FcPatternCreate();
            ensure!(!p.is_null(), "FcPatternCreate failed");
            add_object();
            Ok(Pattern { pat: p })
        }
    }

    pub fn add_charset(&mut self, charset: &CharSet) -> anyhow::Result<()> {
        unsafe {
            ensure!(
                FcPatternAddCharSet(
                    self.pat,
                    b"charset\0".as_ptr() as *const c_char,
                    charset.cset
                ) != 0,
                "failed to add charset property"
            );
            Ok(())
        }
    }

    pub fn charset_intersect_count(&self, charset: &CharSet) -> anyhow::Result<u32> {
        unsafe {
            let mut c = ptr::null_mut();
            FcPatternGetCharSet(self.pat, b"charset\0".as_ptr() as *const c_char, 0, &mut c);
            ensure!(!c.is_null(), "pattern has no charset");
            Ok(FcCharSetIntersectCount(c, charset.cset))
        }
    }

    pub fn add_string(&mut self, key: &str, value: &str) -> Result<(), Error> {
        let key = CString::new(key)?;
        let value = CString::new(value)?;
        unsafe {
            ensure!(
                FcPatternAddString(self.pat, key.as_ptr(), value.as_ptr() as *const u8) != 0,
                "failed to add string property {:?} -> {:?}",
                key,
                value
            );
            Ok(())
        }
    }

    #[allow(dead_code)]
    pub fn add_double(&mut self, key: &str, value: f64) -> Result<(), Error> {
        let key = CString::new(key)?;
        unsafe {
            ensure!(
                FcPatternAddDouble(self.pat, key.as_ptr(), value) != 0,
                "failed to set double property {:?} -> {}",
                key,
                value
            );
            Ok(())
        }
    }

    pub fn add_integer(&mut self, key: &str, value: i32) -> Result<(), Error> {
        let key = CString::new(key)?;
        unsafe {
            ensure!(
                FcPatternAddInteger(self.pat, key.as_ptr(), value) != 0,
                "failed to set integer property {:?} -> {}",
                key,
                value
            );
            Ok(())
        }
    }

    pub fn family(&mut self, family: &str) -> Result<(), Error> {
        self.add_string("family", family)
    }

    pub fn monospace(&mut self) -> Result<(), Error> {
        self.add_integer("spacing", FC_MONO)
    }

    pub fn dual(&mut self) -> Result<(), Error> {
        self.add_integer("spacing", FC_DUAL)
    }

    pub fn format(&self, fmt: &str) -> Result<String, Error> {
        let fmt = CString::new(fmt)?;
        unsafe {
            let s = FcPatternFormat(self.pat, fmt.as_ptr() as *const u8);
            ensure!(!s.is_null(), "failed to format pattern");

            let res = CStr::from_ptr(s as *const c_char)
                .to_string_lossy()
                .into_owned();
            FcStrFree(s);
            Ok(res)
        }
    }

    pub fn render_prepare(&self, pat: &Pattern) -> Result<Pattern, Error> {
        unsafe {
            let pat = FcFontRenderPrepare(ptr::null_mut(), self.pat, pat.pat);
            ensure!(!pat.is_null(), "failed to prepare pattern");
            add_object();
            Ok(Pattern { pat })
        }
    }

    pub fn config_substitute(&mut self, match_kind: MatchKind) -> Result<(), Error> {
        unsafe {
            ensure!(
                FcConfigSubstitute(ptr::null_mut(), self.pat, mem::transmute(match_kind)) != 0,
                "FcConfigSubstitute failed"
            );
            Ok(())
        }
    }

    pub fn default_substitute(&mut self) {
        unsafe {
            FcDefaultSubstitute(self.pat);
        }
    }

    pub fn list(&self) -> anyhow::Result<FontSet> {
        log::trace!("listing: {:?}", self);
        unsafe {
            // This defines the fields that are retrieved
            let oset = FcObjectSetCreate();
            ensure!(!oset.is_null(), "FcObjectSetCreate failed");
            FcObjectSetAdd(oset, b"family\0".as_ptr() as *const c_char);
            FcObjectSetAdd(oset, b"file\0".as_ptr() as *const c_char);
            FcObjectSetAdd(oset, b"index\0".as_ptr() as *const c_char);
            FcObjectSetAdd(oset, b"charset\0".as_ptr() as *const c_char);

            let fonts = FcFontList(ptr::null_mut(), self.pat, oset);
            let result = if !fonts.is_null() {
                add_object();
                Ok(FontSet { fonts })
            } else {
                Err(anyhow!("FcFontList failed"))
            };
            FcObjectSetDestroy(oset);
            result
        }
    }

    pub fn get_best_match(&self) -> Result<Self, Error> {
        unsafe {
            let mut res = FcResultWrap(0);
            let best = FcFontMatch(ptr::null_mut(), self.pat, &mut res.0 as *mut _);

            if !best.is_null() {
                add_object();
            }
            if !res.succeeded() {
                Err(res.as_err())
            } else {
                Ok(Pattern { pat: best })
            }
        }
    }

    pub fn sort(&self, trim: bool) -> Result<FontSet, Error> {
        unsafe {
            let mut res = FcResultWrap(0);
            let fonts = FcFontSort(
                ptr::null_mut(),
                self.pat,
                if trim { 1 } else { 0 },
                ptr::null_mut(),
                &mut res.0 as *mut _,
            );

            if !fonts.is_null() {
                add_object();
            }
            res.result(FontSet { fonts })
        }
    }

    pub fn get_file(&self) -> Result<String, Error> {
        self.get_string("file")
    }

    #[allow(dead_code)]
    pub fn get_double(&self, key: &str) -> Result<f64, Error> {
        unsafe {
            let key = CString::new(key)?;
            let mut fval: f64 = 0.0;
            let res = FcResultWrap(FcPatternGetDouble(
                self.pat,
                key.as_ptr(),
                0,
                &mut fval as *mut _,
            ));
            if !res.succeeded() {
                Err(res.as_err())
            } else {
                Ok(fval)
            }
        }
    }

    pub fn get_integer(&self, key: &str) -> Result<c_int, Error> {
        unsafe {
            let key = CString::new(key)?;
            let mut ival: c_int = 0;
            let res = FcResultWrap(FcPatternGetInteger(
                self.pat,
                key.as_ptr(),
                0,
                &mut ival as *mut _,
            ));
            if !res.succeeded() {
                Err(res.as_err())
            } else {
                Ok(ival)
            }
        }
    }

    pub fn get_string(&self, key: &str) -> Result<String, Error> {
        unsafe {
            let key = CString::new(key)?;
            let mut ptr: *mut u8 = ptr::null_mut();
            let res = FcResultWrap(FcPatternGetString(
                self.pat,
                key.as_ptr(),
                0,
                &mut ptr as *mut *mut u8,
            ));
            if !res.succeeded() {
                Err(res.as_err())
            } else {
                Ok(CStr::from_ptr(ptr as *const c_char)
                    .to_string_lossy()
                    .into_owned())
            }
        }
    }
}

impl Drop for Pattern {
    fn drop(&mut self) {
        unsafe {
            FcPatternDestroy(self.pat);
        }
        release_object();
    }
}

impl fmt::Debug for Pattern {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        // unsafe{FcPatternPrint(self.pat);}
        fmt.write_str(
            &self
                .format("Pattern(%{+family,style,weight,slant,spacing,file,index,charset,fontformat{%{=unparse}}})")
                .unwrap(),
        )
    }
}
