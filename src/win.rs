//! Small Win32 helpers: clipboard + a blocking HTTPS GET via WinHTTP.
//! Going through WinHTTP keeps the program dependency-free: no TLS crate, no
//! HTTP crate, and it honours the proxy settings the user already has.

use std::ffi::c_void;
use windows::core::PCWSTR;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Foundation::HWND;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows::Win32::Graphics::Dwm::{
    DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND, DWMWCP_ROUND,
    DwmSetWindowAttribute,
};
use windows::Win32::Networking::WinHttp::*;
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    IsZoomed, NCCALCSIZE_PARAMS, WM_NCCALCSIZE,
};

/* --------------------------------------------------------------- 窗口外观 */

/// A maximized resizable window is normally placed a frame-width outside the
/// monitor so the frame hides itself. Without decorations that just clips our
/// own title bar, so pin the client area to the monitor work area instead.
pub fn keep_maximized_client_area(hwnd: HWND) {
    unsafe {
        let _ = SetWindowSubclass(hwnd, Some(subclass_proc), 1, 0);
    }
}

unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    unsafe {
        if message == WM_NCCALCSIZE && wparam.0 == 1 && IsZoomed(hwnd).as_bool() {
            let params = lparam.0 as *mut NCCALCSIZE_PARAMS;
            if !params.is_null() {
                let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
                let mut info = MONITORINFO {
                    cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                    ..Default::default()
                };
                if GetMonitorInfoW(monitor, &mut info).as_bool() {
                    (*params).rgrc[0] = info.rcWork;
                    return LRESULT(0);
                }
            }
        }
        DefSubclassProc(hwnd, message, wparam, lparam)
    }
}

/// Rounded corners on Windows 11 plus a hairline border in the app's own
/// palette, so the frameless window still reads as a window.
pub fn dress_window(hwnd: HWND, dark: bool, maximized: bool) {
    let corner = if maximized { DWMWCP_DONOTROUND } else { DWMWCP_ROUND };
    // COLORREF is 0x00BBGGRR. Matches --rule-2: #3c4048 dark, #cfcfc9 light.
    let border: u32 = if dark { 0x0048403C } else { 0x00C9CFCF };
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner as *const _ as *const c_void,
            std::mem::size_of::<i32>() as u32,
        );
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &border as *const _ as *const c_void,
            std::mem::size_of::<u32>() as u32,
        );
        let dark_mode: i32 = if dark { 1 } else { 0 };
        let _ = DwmSetWindowAttribute(
            hwnd,
            windows::Win32::Graphics::Dwm::DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark_mode as *const _ as *const c_void,
            std::mem::size_of::<i32>() as u32,
        );
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/* ------------------------------------------------------------------ clipboard */

pub fn set_clipboard(text: &str) -> Result<(), String> {
    for attempt in 0..8 {
        match unsafe { OpenClipboard(None) } {
            Ok(()) => break,
            Err(_) if attempt < 7 => std::thread::sleep(std::time::Duration::from_millis(30)),
            Err(err) => return Err(format!("剪贴板被其他程序占用（{err}）")),
        }
    }

    let outcome = (|| -> Result<(), String> {
        unsafe { EmptyClipboard() }.map_err(|e| format!("清空剪贴板失败：{e}"))?;
        let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = utf16.len() * std::mem::size_of::<u16>();
        let handle = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes) }
            .map_err(|e| format!("分配剪贴板内存失败：{e}"))?;
        let target = unsafe { GlobalLock(handle) };
        if target.is_null() {
            return Err("锁定剪贴板内存失败".into());
        }
        unsafe {
            std::ptr::copy_nonoverlapping(utf16.as_ptr() as *const u8, target as *mut u8, bytes);
            let _ = GlobalUnlock(handle);
        }
        unsafe { SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(handle.0))) }
            .map_err(|e| format!("写入剪贴板失败：{e}"))?;
        Ok(())
    })();

    unsafe {
        let _ = CloseClipboard();
    }
    outcome
}

/* ----------------------------------------------------------------- http get */

struct Target {
    host: String,
    path: String,
    port: u16,
    secure: bool,
}

fn split_url(url: &str) -> Result<Target, String> {
    let (scheme, rest) = url.split_once("://").ok_or("地址格式不对")?;
    let secure = match scheme.to_ascii_lowercase().as_str() {
        "https" => true,
        "http" => false,
        _ => return Err(format!("不支持的协议：{scheme}")),
    };
    let (authority, path) = match rest.find('/') {
        Some(index) => (&rest[..index], &rest[index..]),
        None => (rest, "/"),
    };
    let mut parts = authority.split(':');
    let host = parts.next().unwrap_or_default().to_string();
    if host.is_empty() {
        return Err("地址里没有主机名".into());
    }
    let port = parts
        .next()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(if secure { 443 } else { 80 });
    Ok(Target {
        host,
        path: path.to_string(),
        port,
        secure,
    })
}

/// Blocking HTTPS GET. Returns the response body as UTF-8 text.
pub fn http_get(url: &str, timeout_ms: i32, headers: &str) -> Result<String, String> {
    let target = split_url(url)?;
    let agent = wide("MoyueMD/1.0");
    let host = wide(&target.host);
    let path = wide(&target.path);
    let header_text: Vec<u16> = headers.encode_utf16().collect();

    unsafe {
        let session = WinHttpOpen(
            PCWSTR(agent.as_ptr()),
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        );
        if session.is_null() {
            return Err("无法初始化网络会话".into());
        }
        let _ = WinHttpSetTimeouts(session, timeout_ms, timeout_ms, timeout_ms, timeout_ms);

        let result = (|| -> Result<String, String> {
            let connection = WinHttpConnect(session, PCWSTR(host.as_ptr()), target.port, 0);
            if connection.is_null() {
                return Err(format!("连不上 {}", target.host));
            }

            let request = WinHttpOpenRequest(
                connection,
                windows::core::w!("GET"),
                PCWSTR(path.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                std::ptr::null(),
                if target.secure {
                    WINHTTP_FLAG_SECURE
                } else {
                    WINHTTP_OPEN_REQUEST_FLAGS(0)
                },
            );
            if request.is_null() {
                let _ = WinHttpCloseHandle(connection);
                return Err("无法创建请求".into());
            }

            let send = (|| -> Result<String, String> {
                WinHttpSendRequest(
                    request,
                    if header_text.is_empty() {
                        None
                    } else {
                        Some(&header_text[..])
                    },
                    None,
                    0,
                    0,
                    0,
                )
                .map_err(|e| format!("发送请求失败：{e}"))?;
                WinHttpReceiveResponse(request, std::ptr::null_mut())
                    .map_err(|e| format!("读取响应失败：{e}"))?;

                let mut status: u32 = 0;
                let mut length = std::mem::size_of::<u32>() as u32;
                let _ = WinHttpQueryHeaders(
                    request,
                    WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                    PCWSTR::null(),
                    Some(&mut status as *mut u32 as *mut c_void),
                    &mut length,
                    std::ptr::null_mut(),
                );

                let mut body: Vec<u8> = Vec::new();
                loop {
                    let mut buffer = [0u8; 8192];
                    let mut read: u32 = 0;
                    if WinHttpReadData(
                        request,
                        buffer.as_mut_ptr() as *mut c_void,
                        buffer.len() as u32,
                        &mut read,
                    )
                    .is_err()
                    {
                        break;
                    }
                    if read == 0 {
                        break;
                    }
                    body.extend_from_slice(&buffer[..read as usize]);
                    if body.len() > 8 * 1024 * 1024 {
                        break;
                    }
                }

                if !(200..300).contains(&status) {
                    return Err(format!("服务返回 HTTP {status}"));
                }
                String::from_utf8(body).map_err(|_| "响应不是 UTF-8 文本".to_string())
            })();

            let _ = WinHttpCloseHandle(request);
            let _ = WinHttpCloseHandle(connection);
            send
        })();

        let _ = WinHttpCloseHandle(session);
        result
    }
}
