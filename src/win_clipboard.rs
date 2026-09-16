//! Windows 系统剪贴板后端
//!
//! dear-imgui-rs 只内置 Dummy 剪贴板（Winit 平台层不再自动接入系统剪贴板），
//! 这里通过 Win32 API 提供 UTF-16 文本的读写，保证输入框 Ctrl+C/V 与
//! 值表的"从剪贴板导入"功能可用。

use dear_imgui_rs::ClipboardBackend;

#[cfg(windows)]
pub struct WinClipboardBackend;

#[cfg(windows)]
mod sys {
    pub type HANDLE = *mut core::ffi::c_void;
    pub type HWND = *mut core::ffi::c_void;
    pub type BOOL = i32;
    pub type UINT = u32;

    #[link(name = "user32")]
    unsafe extern "system" {
        pub fn OpenClipboard(hwnd: HWND) -> BOOL;
        pub fn CloseClipboard() -> BOOL;
        pub fn EmptyClipboard() -> BOOL;
        pub fn GetClipboardData(format: UINT) -> HANDLE;
        pub fn SetClipboardData(format: UINT, mem: HANDLE) -> HANDLE;
        pub fn IsClipboardFormatAvailable(format: UINT) -> BOOL;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn GlobalLock(mem: HANDLE) -> *mut core::ffi::c_void;
        pub fn GlobalUnlock(mem: HANDLE) -> BOOL;
        pub fn GlobalAlloc(flags: UINT, bytes: usize) -> HANDLE;
        pub fn GlobalFree(mem: HANDLE) -> HANDLE;
    }
}

#[cfg(windows)]
const CF_UNICODETEXT: u32 = 13;
#[cfg(windows)]
const GMEM_MOVEABLE: u32 = 0x0002;

#[cfg(windows)]
impl ClipboardBackend for WinClipboardBackend {
    fn get(&mut self) -> Option<String> {
        unsafe {
            if sys::IsClipboardFormatAvailable(CF_UNICODETEXT) == 0 {
                return None;
            }
            if sys::OpenClipboard(std::ptr::null_mut()) == 0 {
                return None;
            }

            let result = (|| {
                let handle = sys::GetClipboardData(CF_UNICODETEXT);
                if handle.is_null() {
                    return None;
                }
                let ptr = sys::GlobalLock(handle);
                if ptr.is_null() {
                    return None;
                }

                let mut len = 0usize;
                while *(ptr as *const u16).add(len) != 0 {
                    len += 1;
                }
                let slice = std::slice::from_raw_parts(ptr as *const u16, len);
                let text = String::from_utf16_lossy(slice);

                sys::GlobalUnlock(handle);
                Some(text)
            })();

            sys::CloseClipboard();
            result
        }
    }

    fn set(&mut self, value: &str) {
        unsafe {
            if sys::OpenClipboard(std::ptr::null_mut()) == 0 {
                return;
            }

            let mut utf16: Vec<u16> = value.encode_utf16().collect();
            utf16.push(0);

            let bytes = utf16.len() * 2;
            let handle = sys::GlobalAlloc(GMEM_MOVEABLE, bytes);
            if !handle.is_null() {
                let ptr = sys::GlobalLock(handle);
                if !ptr.is_null() {
                    std::ptr::copy_nonoverlapping(utf16.as_ptr(), ptr as *mut u16, utf16.len());
                    sys::GlobalUnlock(handle);
                    sys::EmptyClipboard();
                    // 成功后句柄归系统所有；失败则手动释放
                    if sys::SetClipboardData(CF_UNICODETEXT, handle).is_null() {
                        sys::GlobalFree(handle);
                    }
                } else {
                    sys::GlobalFree(handle);
                }
            }

            sys::CloseClipboard();
        }
    }
}

#[cfg(not(windows))]
pub struct WinClipboardBackend;

#[cfg(not(windows))]
impl ClipboardBackend for WinClipboardBackend {
    fn get(&mut self) -> Option<String> {
        None
    }

    fn set(&mut self, _value: &str) {}
}
