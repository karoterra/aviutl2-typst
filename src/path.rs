use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::sync::LazyLock;

use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::{
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
    GetModuleFileNameW, GetModuleHandleExW,
};
use windows::core::PCWSTR;

pub static DLL_DIR: LazyLock<Option<PathBuf>> =
    LazyLock::new(|| get_dll_path().and_then(|path| path.parent().map(|p| p.to_path_buf())));

pub fn get_dll_path() -> Option<PathBuf> {
    unsafe {
        let mut module = HMODULE::default();
        let addr = get_dll_path as *const () as *const u16;
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            PCWSTR(addr),
            &mut module,
        )
        .ok()?;

        let mut buf = vec![0u16; 260];
        let len = GetModuleFileNameW(Some(module), &mut buf) as usize;
        if len == 0 {
            return None;
        }
        Some(PathBuf::from(OsString::from_wide(&buf[..len])))
    }
}

pub fn get_package_dir() -> Option<PathBuf> {
    DLL_DIR.as_ref().map(|dll_dir| dll_dir.join("package"))
}

pub fn get_package_cache_dir() -> Option<PathBuf> {
    DLL_DIR
        .as_ref()
        .map(|dll_dir| dll_dir.join("package_cache"))
}

pub fn get_aviutl2_font_dir() -> PathBuf {
    aviutl2::config::app_data_path().join("Font")
}
