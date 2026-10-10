//! 保存与导出对话框的起始目录：用户文档目录，或当前文件自己的目录。
//!
//! 不设起始目录时，Windows 的通用对话框会停在进程当前目录——双击 exe 就是
//! exe 旁边，装到 `C:\...\cliapps\` 后新建的 DBC 就全堆在那儿了。

use std::path::{Path, PathBuf};

/// 新建或导出时的起始目录
pub fn documents_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Some(path) = known_folder_documents() {
            return path;
        }
        if let Some(profile) = std::env::var_os("USERPROFILE") {
            let profile = PathBuf::from(profile);
            let documents = profile.join("Documents");
            return if documents.is_dir() {
                documents
            } else {
                profile
            };
        }
    }
    #[cfg(not(windows))]
    {
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            let documents = home.join("Documents");
            return if documents.is_dir() { documents } else { home };
        }
    }
    PathBuf::from(".")
}

/// 窗口已经有文件就回到它自己的目录，没有（新建、从 Excel 生成等）才用文档目录
pub fn save_dir_for(current: &str) -> PathBuf {
    match Path::new(current).parent() {
        Some(parent) if !parent.as_os_str().is_empty() && parent.is_dir() => parent.to_path_buf(),
        _ => documents_dir(),
    }
}

/// FOLDERID_Documents = {FDD39AD0-228F-47F0-AF20-87E5A86E13E7}
#[cfg(windows)]
const FOLDERID_DOCUMENTS: (u32, u16, u16, [u8; 8]) = (
    0xFDD39AD0,
    0x228F,
    0x47F0,
    [0xAF, 0x20, 0x87, 0xE5, 0xA8, 0x6E, 0x13, 0xE7],
);

/// 走 shell 的已知文件夹接口，这样 OneDrive 重定向过的"文档"也能取对
#[cfg(windows)]
fn known_folder_documents() -> Option<PathBuf> {
    #[repr(C)]
    struct Guid {
        low: u32,
        mid: u16,
        long: u16,
        tail: [u8; 8],
    }
    #[link(name = "shell32")]
    unsafe extern "system" {
        fn SHGetKnownFolderPath(
            folder: *const Guid,
            flags: u32,
            token: *const u8,
            path: *mut *mut u16,
        ) -> i32;
    }
    #[link(name = "ole32")]
    unsafe extern "system" {
        fn CoTaskMemFree(pointer: *const u8);
    }

    let (low, mid, long, tail) = FOLDERID_DOCUMENTS;
    let folder = Guid {
        low,
        mid,
        long,
        tail,
    };
    let mut wide: *mut u16 = std::ptr::null_mut();
    unsafe {
        // S_OK 是 0；失败或没给指针就交给调用方兜底
        if SHGetKnownFolderPath(&folder, 0, std::ptr::null(), &mut wide) != 0 || wide.is_null() {
            return None;
        }
        let mut length = 0;
        while *wide.add(length) != 0 {
            length += 1;
        }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(wide, length));
        CoTaskMemFree(wide as *const u8);
        let path = PathBuf::from(text);
        path.is_dir().then_some(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_documents_folder_is_a_real_directory_outside_the_build() {
        let documents = documents_dir();
        assert!(documents.is_dir(), "{}", documents.display());
        assert!(
            !documents.ends_with(Path::new("target").join("release")),
            "save dialogs must not start next to the exe: {}",
            documents.display()
        );
    }

    #[test]
    fn an_open_file_starts_the_dialog_in_its_own_folder() {
        let dir = std::env::temp_dir().join(format!("roxy-dbc-paths-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let file = dir.join("motor.dbc");
        std::fs::write(&file, "VERSION \"\"\n").expect("write fixture");

        assert_eq!(save_dir_for(&file.to_string_lossy()), dir);
        // 新建还没落盘的文档没有自己的目录，退回文档目录
        assert_eq!(save_dir_for("untitled.dbc"), documents_dir());
        std::fs::remove_dir_all(&dir).ok();
    }
}
