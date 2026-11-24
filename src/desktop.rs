//! Mọi thứ liên quan tới Win32: gắn cửa sổ xuống dưới icon desktop (WorkerW),
//! phát hiện khi nào nên tạm dừng, và trả lại hình nền gốc khi thoát.

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{BOOL, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND, LPARAM, POINT, RECT, TRUE, WPARAM};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MapWindowPoints, MonitorFromPoint, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONULL, MONITOR_DEFAULTTOPRIMARY};
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Shell::{SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN};
use windows::Win32::UI::WindowsAndMessaging::*;

/// Vùng màn hình chính, tính bằng pixel thật.
#[allow(dead_code)]
pub fn primary_monitor_rect() -> RECT {
    unsafe {
        let mon = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        let _ = GetMonitorInfoW(mon, &mut mi);
        mi.rcMonitor
    }
}

/// Chỉ cho chạy một bản. Trả về false nếu app đã chạy rồi.
pub fn single_instance() -> bool {
    unsafe {
        let h: windows::core::Result<HANDLE> = CreateMutexW(None, true, w!("Local\\SeasonDesktopSingleton"));
        // Cố tình không đóng handle: mutex sống tới khi tiến trình kết thúc.
        h.is_ok() && GetLastError() != ERROR_ALREADY_EXISTS
    }
}

unsafe extern "system" fn find_old_workerw(top: HWND, lp: LPARAM) -> BOOL {
    if FindWindowExW(top, HWND::default(), w!("SHELLDLL_DefView"), PCWSTR::null()).is_ok() {
        if let Ok(worker) = FindWindowExW(HWND::default(), top, w!("WorkerW"), PCWSTR::null()) {
            *(lp.0 as *mut HWND) = worker;
        }
    }
    TRUE
}

/// Một màn hình đang cắm.
#[derive(Clone, Debug)]
pub struct Monitor {
    pub rect: RECT,
    pub primary: bool,
    /// Tên thiết bị, ví dụ \\.\DISPLAY1 – dùng để nhớ cài đặt riêng của màn hình.
    pub name: String,
    /// Tần số quét (Hz)
    pub hz: u32,
}

impl Monitor {
    pub fn size(&self) -> (u32, u32) {
        ((self.rect.right - self.rect.left) as u32, (self.rect.bottom - self.rect.top) as u32)
    }
    pub fn portrait(&self) -> bool {
        let (w, h) = self.size();
        h > w
    }
}

unsafe extern "system" fn monitor_cb(h: windows::Win32::Graphics::Gdi::HMONITOR, _: windows::Win32::Graphics::Gdi::HDC, _: *mut RECT, lp: LPARAM) -> BOOL {
    let v = &mut *(lp.0 as *mut Vec<Monitor>);
    use windows::Win32::Graphics::Gdi::{EnumDisplaySettingsW, DEVMODEW, ENUM_CURRENT_SETTINGS, MONITORINFOEXW};
    let mut mi = MONITORINFOEXW::default();
    mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    if GetMonitorInfoW(h, &mut mi as *mut MONITORINFOEXW as *mut MONITORINFO).as_bool() {
        let n = mi.szDevice.iter().position(|&c| c == 0).unwrap_or(mi.szDevice.len());
        let name = String::from_utf16_lossy(&mi.szDevice[..n]);
        let mut dm = DEVMODEW { dmSize: std::mem::size_of::<DEVMODEW>() as u16, ..Default::default() };
        let hz = if EnumDisplaySettingsW(PCWSTR(mi.szDevice.as_ptr()), ENUM_CURRENT_SETTINGS, &mut dm).as_bool() { dm.dmDisplayFrequency } else { 60 };
        v.push(Monitor { rect: mi.monitorInfo.rcMonitor, primary: mi.monitorInfo.dwFlags & 1 != 0, name, hz: hz.max(30) });
    }
    TRUE
}

/// Tất cả màn hình, màn hình chính đứng đầu.
pub fn monitors() -> Vec<Monitor> {
    let mut v: Vec<Monitor> = Vec::new();
    unsafe {
        let _ = windows::Win32::Graphics::Gdi::EnumDisplayMonitors(
            windows::Win32::Graphics::Gdi::HDC::default(),
            None,
            Some(monitor_cb),
            LPARAM(&mut v as *mut Vec<Monitor> as isize),
        );
    }
    v.sort_by_key(|m| (!m.primary, m.rect.left, m.rect.top));
    v
}

/// Gắn `child` vào lớp nằm giữa hình nền gốc và icon desktop, phủ đúng màn hình `r`.
/// Hỗ trợ cả cấu trúc cũ (Windows 10 / 11 trước 24H2) và mới (24H2 trở đi).
pub fn attach(child: HWND, r: RECT) -> Result<&'static str, String> {
    unsafe {
        let progman = FindWindowW(w!("Progman"), PCWSTR::null()).map_err(|e| format!("không tìm thấy Progman: {e}"))?;

        // Nhờ Explorer tạo lớp WorkerW nằm sau icon.
        let mut res = 0usize;
        let _ = SendMessageTimeoutW(progman, 0x052C, WPARAM(0xD), LPARAM(0x1), SMTO_NORMAL, 1000, Some(&mut res));
        let _ = SendMessageTimeoutW(progman, 0x052C, WPARAM(0), LPARAM(0), SMTO_NORMAL, 1000, Some(&mut res));

        // Biến cửa sổ thành cửa sổ con trước khi đổi cha.
        let style = (WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WS_CLIPCHILDREN).0 as isize;
        SetWindowLongPtrW(child, GWL_STYLE, style);
        let ex = (WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW).0 as isize;
        SetWindowLongPtrW(child, GWL_EXSTYLE, ex);

        let defview = FindWindowExW(progman, HWND::default(), w!("SHELLDLL_DefView"), PCWSTR::null()).ok();

        let (parent, insert_after, layout) = if let Some(defview) = defview {
            // Cấu trúc mới (24H2+): Progman chứa cả SHELLDLL_DefView (icon) và WorkerW (hình nền).
            // Đặt cửa sổ của mình ngay dưới lớp icon, trên WorkerW.
            (progman, defview, "24H2+ (Progman > DefView / app / WorkerW)")
        } else {
            // Cấu trúc cũ: WorkerW là cửa sổ cấp cao nằm ngay sau cửa sổ chứa icon.
            let mut worker = HWND::default();
            let _ = EnumWindows(Some(find_old_workerw), LPARAM(&mut worker as *mut HWND as isize));
            if worker.is_invalid() {
                (progman, HWND_BOTTOM, "dự phòng (gắn thẳng vào Progman)")
            } else {
                (worker, HWND_TOP, "cổ điển (WorkerW)")
            }
        };

        SetParent(child, parent).map_err(|e| format!("SetParent lỗi: {e}"))?;
        place(child, parent, insert_after, r);

        // Ở cấu trúc mới, đẩy WorkerW (hình nền gốc) xuống DƯỚI CÙNG, để nó không che
        // hình nền của các màn hình khác (mỗi màn hình có một cửa sổ con riêng).
        if defview.is_some() {
            if let Ok(worker) = FindWindowExW(progman, HWND::default(), w!("WorkerW"), PCWSTR::null()) {
                let _ = SetWindowPos(worker, HWND_BOTTOM, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            }
        }
        Ok(layout)
    }
}

/// Đẩy lớp hình nền gốc của Windows (WorkerW trong Progman, kiểu 24H2+) xuống dưới cùng, để nó
/// không che hình nền của app. Explorer đưa lớp này lên lại khi đổi ảnh nền (hiệu ứng chuyển ảnh).
pub fn restack() {
    unsafe {
        let Ok(progman) = FindWindowW(w!("Progman"), PCWSTR::null()) else { return };
        if FindWindowExW(progman, HWND::default(), w!("SHELLDLL_DefView"), PCWSTR::null()).is_err() {
            return;
        }
        let mut prev = HWND::default();
        while let Ok(worker) = FindWindowExW(progman, prev, w!("WorkerW"), PCWSTR::null()) {
            let _ = SetWindowPos(worker, HWND_BOTTOM, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            prev = worker;
        }
    }
}

/// Đặt cửa sổ con phủ đúng màn hình chính (đổi toạ độ màn hình sang toạ độ của cửa sổ cha).
unsafe fn place(child: HWND, parent: HWND, insert_after: HWND, r: RECT) {
    let mut pts = [POINT { x: r.left, y: r.top }, POINT { x: r.right, y: r.bottom }];
    MapWindowPoints(HWND::default(), parent, &mut pts);
    let _ = SetWindowPos(
        child,
        insert_after,
        pts[0].x,
        pts[0].y,
        pts[1].x - pts[0].x,
        pts[1].y - pts[0].y,
        SWP_NOACTIVATE | SWP_SHOWWINDOW,
    );
}

/// Cửa sổ cha còn sống không (Explorer khởi động lại sẽ huỷ nó).
pub fn still_attached(child: HWND) -> bool {
    unsafe { IsWindow(child).as_bool() && !GetParent(child).unwrap_or_default().is_invalid() }
}

/// Có nên tạm dừng vẽ không: game/app toàn màn hình, trình chiếu,
/// hoặc cửa sổ phóng to đang che kín màn hình chính.
#[allow(dead_code)]
pub fn should_pause(fullscreen: bool, maximized: bool) -> bool {
    unsafe {
        if fullscreen {
            if let Ok(state) = SHQueryUserNotificationState() {
                if state == QUNS_BUSY || state == QUNS_RUNNING_D3D_FULL_SCREEN || state == QUNS_PRESENTATION_MODE {
                    return true;
                }
            }
        }
        if maximized {
            let fg = GetForegroundWindow();
            if !fg.is_invalid() && IsZoomed(fg).as_bool() && IsWindowVisible(fg).as_bool() {
                let primary = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
                if MonitorFromWindow(fg, MONITOR_DEFAULTTONULL) == primary {
                    return true;
                }
            }
        }
        false
    }
}

unsafe extern "system" fn covered_cb(h: HWND, lp: LPARAM) -> BOOL {
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    let v = &mut *(lp.0 as *mut Vec<(RECT, bool)>);
    if !IsWindowVisible(h).as_bool() || IsIconic(h).as_bool() {
        return TRUE;
    }
    let mut cloaked = 0u32;
    let _ = DwmGetWindowAttribute(h, DWMWA_CLOAKED, &mut cloaked as *mut u32 as *mut _, 4);
    if cloaked != 0 {
        return TRUE;
    }
    let ex = GetWindowLongPtrW(h, GWL_EXSTYLE) as u32;
    if ex & (WS_EX_TOOLWINDOW.0 | WS_EX_LAYERED.0 | WS_EX_TRANSPARENT.0) != 0 && !IsZoomed(h).as_bool() {
        return TRUE;
    }
    let mut cls = [0u16; 64];
    let n = GetClassNameW(h, &mut cls);
    let class = String::from_utf16_lossy(&cls[..n.max(0) as usize]);
    if matches!(class.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd") {
        return TRUE;
    }
    let mut r = RECT::default();
    if GetWindowRect(h, &mut r).is_ok() {
        v.push((r, IsZoomed(h).as_bool()));
    }
    TRUE
}

/// Vị trí các cửa sổ đang hiện (và có đang phóng to không) – lấy một lần cho mọi màn hình.
pub fn visible_windows() -> Vec<(RECT, bool)> {
    let mut wins: Vec<(RECT, bool)> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(covered_cb), LPARAM(&mut wins as *mut Vec<(RECT, bool)> as isize));
    }
    wins
}

/// Màn hình `m` có đang bị che kín không (cửa sổ phóng to hoặc toàn màn hình nằm trên nó)?
/// Mỗi màn hình được xét riêng, không phụ thuộc cửa sổ nào đang được chọn.
#[allow(dead_code)]
pub fn monitor_covered(m: RECT, maximized: bool, fullscreen: bool) -> bool {
    covered_by(m, &visible_windows(), maximized, fullscreen)
}

pub fn covered_by(m: RECT, wins: &[(RECT, bool)], maximized: bool, fullscreen: bool) -> bool {
    wins.iter().any(|(r, zoomed)| {
        // tâm cửa sổ nằm trên màn hình này
        let (cx, cy) = ((r.left + r.right) / 2, (r.top + r.bottom) / 2);
        if cx < m.left || cx >= m.right || cy < m.top || cy >= m.bottom {
            return false;
        }
        let full = r.left <= m.left && r.top <= m.top && r.right >= m.right && r.bottom >= m.bottom;
        (maximized && *zoomed) || (fullscreen && full)
    })
}

/// Đang chạy pin (không cắm sạc)?
pub fn on_battery() -> bool {
    unsafe {
        let mut s = SYSTEM_POWER_STATUS::default();
        GetSystemPowerStatus(&mut s).is_ok() && s.ACLineStatus == 0
    }
}

#[derive(Clone, Copy)]
pub struct LocalTime {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    /// 0 = Chủ Nhật
    pub weekday: u32,
    pub hour: u32,
    pub minute: u32,
}

pub fn local_time() -> LocalTime {
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    LocalTime {
        year: t.wYear as i32,
        month: t.wMonth as u32,
        day: t.wDay as u32,
        weekday: t.wDayOfWeek as u32,
        hour: t.wHour as u32,
        minute: t.wMinute as u32,
    }
}

const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const RUN_NAME: PCWSTR = w!("SeasonDesktop");

/// Có đang bật khởi động cùng Windows không (chỉ đọc khoá của người dùng hiện tại).
pub fn autostart_enabled() -> bool {
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};
    unsafe { RegGetValueW(HKEY_CURRENT_USER, RUN_KEY, RUN_NAME, RRF_RT_REG_SZ, None, None, None).is_ok() }
}

/// Bật/tắt khởi động cùng Windows. Chỉ ghi vào HKCU\...\Run, không cần quyền admin,
/// không đụng tới thiết lập hệ thống nào khác.
pub fn set_autostart(on: bool) -> Result<(), String> {
    use windows::Win32::System::Registry::*;
    unsafe {
        let mut key = HKEY::default();
        let r = RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY, 0, KEY_SET_VALUE, &mut key);
        if r.is_err() {
            return Err(format!("không mở được khoá Run: {r:?}"));
        }
        let res = if on {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let cmd = format!("\"{}\" --minimized", exe.display());
            let wide: Vec<u16> = cmd.encode_utf16().chain(std::iter::once(0)).collect();
            let bytes = std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2);
            RegSetValueExW(key, RUN_NAME, 0, REG_SZ, Some(bytes))
        } else {
            RegDeleteValueW(key, RUN_NAME)
        };
        let _ = RegCloseKey(key);
        if on && res.is_err() {
            return Err(format!("không ghi được: {res:?}"));
        }
        Ok(())
    }
}

fn original_path_file() -> std::path::PathBuf {
    crate::config::dir().join("original-wallpaper.txt")
}

/// Đường dẫn hình nền Windows hiện tại.
#[allow(dead_code)]
pub fn current_wallpaper() -> String {
    let mut buf = [0u16; 1024];
    unsafe {
        let _ = SystemParametersInfoW(SPI_GETDESKWALLPAPER, buf.len() as u32, Some(buf.as_mut_ptr() as *mut _), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0));
    }
    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..n])
}

/// Đặt hình nền Windows (lưu lại, như khi bạn đổi trong Cài đặt).
pub fn set_wallpaper(path: &str) -> bool {
    let mut w: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        SystemParametersInfoW(SPI_SETDESKWALLPAPER, 0, Some(w.as_mut_ptr() as *mut _), SPIF_UPDATEINIFILE | SPIF_SENDCHANGE).is_ok()
    }
}

/// Đặt ảnh tĩnh riêng cho từng màn hình (ngang/dọc) qua IDesktopWallpaper.
/// Lần đầu thì ghi lại hình nền gốc của từng màn hình để khôi phục được.
pub fn set_still_wallpapers(items: &[(RECT, String)]) -> bool {
    use windows::Win32::System::Com::*;
    use windows::Win32::UI::Shell::{DesktopWallpaper, IDesktopWallpaper};
    let f = original_path_file();
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let Ok(dw) = CoCreateInstance::<_, IDesktopWallpaper>(&DesktopWallpaper, None, CLSCTX_ALL) else {
            return items.first().map(|(_, p)| set_wallpaper(p)).unwrap_or(false);
        };
        let n = dw.GetMonitorDevicePathCount().unwrap_or(0);
        let save_orig = !f.exists();
        let mut orig = String::new();
        let mut ok = false;
        for i in 0..n {
            let Ok(id) = dw.GetMonitorDevicePathAt(i) else { continue };
            let idc = PCWSTR(id.0);
            if save_orig {
                if let Ok(cur) = dw.GetWallpaper(idc) {
                    orig += &format!("{}{TAB}{}\n", id.to_string().unwrap_or_default(), cur.to_string().unwrap_or_default());
                    CoTaskMemFree(Some(cur.0 as *const _));
                }
            }
            if let Ok(r) = dw.GetMonitorRECT(idc) {
                if let Some((_, path)) = items.iter().find(|(m, _)| m.left == r.left && m.top == r.top && m.right == r.right && m.bottom == r.bottom) {
                    let w: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
                    ok |= dw.SetWallpaper(idc, PCWSTR(w.as_ptr())).is_ok();
                }
            }
            CoTaskMemFree(Some(id.0 as *const _));
        }
        if save_orig && !orig.is_empty() {
            let _ = std::fs::create_dir_all(crate::config::dir());
            let _ = std::fs::write(&f, orig);
        }
        ok
    }
}

const TAB: char = '\u{9}';

/// Trả lại hình nền gốc cho riêng một màn hình (nếu app từng ghi lại hình gốc của nó).
pub fn restore_original_wallpaper_for(rect: RECT) {
    use windows::Win32::System::Com::*;
    use windows::Win32::UI::Shell::{DesktopWallpaper, IDesktopWallpaper};
    let Ok(text) = std::fs::read_to_string(original_path_file()) else { return };
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let Ok(dw) = CoCreateInstance::<_, IDesktopWallpaper>(&DesktopWallpaper, None, CLSCTX_ALL) else { return };
        for line in text.lines() {
            let Some((id, path)) = line.split_once(TAB) else { continue };
            let idw: Vec<u16> = id.encode_utf16().chain(std::iter::once(0)).collect();
            if let Ok(r) = dw.GetMonitorRECT(PCWSTR(idw.as_ptr())) {
                if r.left == rect.left && r.top == rect.top && r.right == rect.right && r.bottom == rect.bottom {
                    let pw: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
                    let _ = dw.SetWallpaper(PCWSTR(idw.as_ptr()), PCWSTR(pw.as_ptr()));
                }
            }
        }
    }
}

/// Trả lại hình nền gốc của từng màn hình (nếu app từng đặt ảnh tĩnh).
pub fn restore_original_wallpaper() {
    use windows::Win32::System::Com::*;
    use windows::Win32::UI::Shell::{DesktopWallpaper, IDesktopWallpaper};
    let f = original_path_file();
    let Ok(text) = std::fs::read_to_string(&f) else {
        restore_wallpaper();
        return;
    };
    let mut ok = false;
    if !text.contains(TAB) {
        // file kiểu cũ: chỉ có một đường dẫn
        ok = set_wallpaper(text.trim());
    } else {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            if let Ok(dw) = CoCreateInstance::<_, IDesktopWallpaper>(&DesktopWallpaper, None, CLSCTX_ALL) {
                for line in text.lines() {
                    if let Some((id, path)) = line.split_once(TAB) {
                        let idw: Vec<u16> = id.encode_utf16().chain(std::iter::once(0)).collect();
                        let pw: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
                        ok |= dw.SetWallpaper(PCWSTR(idw.as_ptr()), PCWSTR(pw.as_ptr())).is_ok();
                    }
                }
            }
        }
    }
    if ok {
        let _ = std::fs::remove_file(f);
    }
}

/// Áp lại hình nền hiện tại để Windows vẽ lại desktop sau khi app thoát.
pub fn restore_wallpaper() {
    unsafe {
        let mut buf = [0u16; 512];
        if SystemParametersInfoW(SPI_GETDESKWALLPAPER, buf.len() as u32, Some(buf.as_mut_ptr() as *mut _), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0)).is_ok() {
            let _ = SystemParametersInfoW(SPI_SETDESKWALLPAPER, 0, Some(buf.as_mut_ptr() as *mut _), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0));
        }
    }
}
