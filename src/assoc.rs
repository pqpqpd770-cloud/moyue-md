//! Register the app as a handler for Markdown files.
//!
//! Everything is written under HKEY_CURRENT_USER, so no administrator rights are
//! needed and nothing is changed for other users. Windows 10/11 keeps a separate
//! per-user choice (`FileExts\.md\UserChoice`, protected by a hash) that an app
//! cannot set programmatically — so we register properly, set the legacy default
//! as a best effort, and report honestly when Windows still points elsewhere.

use std::path::Path;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_NONE, REG_OPTION_NON_VOLATILE, REG_SZ,
    RegCloseKey, RegCreateKeyExW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_FLAGS, SHChangeNotify};
use windows::core::PCWSTR;

const PROGID: &str = "MoyueMD.md";
const EXTENSIONS: [&str; 5] = ["md", "markdown", "mdown", "mkd", "mkdn"];
/// SHCNF_FLUSH: make Explorer pick the change up immediately.
const SHCNF_FLUSH: u32 = 0x1000;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/* ------------------------------------------------------------------ 写注册表 */

unsafe fn create_key(root: HKEY, subkey: &str) -> Result<HKEY, String> {
    let name = wide(subkey);
    let mut key = HKEY::default();
    let status = unsafe {
        RegCreateKeyExW(
            root,
            PCWSTR(name.as_ptr()),
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE | KEY_READ,
            None,
            &mut key,
            None,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(format!("写入注册表失败（{}）", status.0));
    }
    Ok(key)
}

unsafe fn set_string(root: HKEY, subkey: &str, value: &str) -> Result<(), String> {
    let key = unsafe { create_key(root, subkey) }?;
    let name = wide("");
    let data: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = unsafe {
        std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * std::mem::size_of::<u16>())
    };
    let status =
        unsafe { RegSetValueExW(key, PCWSTR(name.as_ptr()), None, REG_SZ, Some(bytes)) };
    unsafe {
        let _ = RegCloseKey(key);
    }
    if status != ERROR_SUCCESS {
        return Err(format!("写入注册表失败（{}）", status.0));
    }
    Ok(())
}

unsafe fn add_progid_to(root: HKEY, subkey: &str) -> Result<(), String> {
    let key = unsafe { create_key(root, subkey) }?;
    let name = wide(PROGID);
    let status = unsafe {
        RegSetValueExW(key, PCWSTR(name.as_ptr()), None, REG_NONE, None)
    };
    unsafe {
        let _ = RegCloseKey(key);
    }
    if status != ERROR_SUCCESS {
        return Err(format!("写入注册表失败（{}）", status.0));
    }
    Ok(())
}

/// Register the handler for every extension we know about and notify the shell.
pub fn register(exe: &Path) -> Result<(), String> {
    let exe = exe.to_string_lossy().to_string();
    let command = format!("\"{exe}\" \"%1\"");

    unsafe {
        set_string(
            HKEY_CURRENT_USER,
            &format!("Software\\Classes\\{PROGID}"),
            "Markdown 文档",
        )?;
        set_string(
            HKEY_CURRENT_USER,
            &format!("Software\\Classes\\{PROGID}\\DefaultIcon"),
            &format!("{exe},0"),
        )?;
        set_string(
            HKEY_CURRENT_USER,
            &format!("Software\\Classes\\{PROGID}\\shell\\open\\command"),
            &command,
        )?;

        for ext in EXTENSIONS {
            add_progid_to(
                HKEY_CURRENT_USER,
                &format!("Software\\Classes\\.{ext}\\OpenWithProgids"),
            )?;
            // Best effort: ignored by Explorer when a UserChoice already exists.
            set_string(HKEY_CURRENT_USER, &format!("Software\\Classes\\.{ext}"), PROGID)?;
        }
    }

    unsafe {
        SHChangeNotify(
            SHCNE_ASSOCCHANGED,
            SHCNF_FLAGS(SHCNF_FLUSH),
            None,
            None,
        );
    }
    Ok(())
}

/* ------------------------------------------------------------------ 读当前默认 */

/// The program Windows currently uses for `.md`, if the user (or the system) has
/// pinned one. Returns the ProgId, e.g. `MoyueMD.md` or `Applications\Code.exe`.
pub fn current_default() -> Option<String> {
    unsafe {
        let subkey = wide("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\FileExts\\.md\\UserChoice");
        let mut key = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, PCWSTR(subkey.as_ptr()), None, KEY_READ, &mut key)
            != ERROR_SUCCESS
        {
            return None;
        }

        let name = wide("ProgId");
        let mut kind = REG_SZ;
        let mut size: u32 = 0;
        let probe = RegQueryValueExW(
            key,
            PCWSTR(name.as_ptr()),
            None,
            Some(&mut kind),
            None,
            Some(&mut size),
        );
        if probe != ERROR_SUCCESS || size == 0 {
            let _ = RegCloseKey(key);
            return None;
        }

        let mut buffer = vec![0u8; size as usize];
        let status = RegQueryValueExW(
            key,
            PCWSTR(name.as_ptr()),
            None,
            None,
            Some(buffer.as_mut_ptr()),
            Some(&mut size),
        );
        let _ = RegCloseKey(key);
        if status != ERROR_SUCCESS {
            return None;
        }

        let units: Vec<u16> = buffer
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .take_while(|unit| *unit != 0)
            .collect();
        let text = String::from_utf16_lossy(&units);
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }
}

pub fn is_ours(prog_id: &str) -> bool {
    prog_id.eq_ignore_ascii_case(PROGID)
}

/// Human-readable name for whatever Windows currently uses.
pub fn describe(prog_id: &str) -> String {
    if let Some(rest) = prog_id.strip_prefix("Applications\\") {
        return rest.to_string();
    }
    prog_id.to_string()
}
