//! Giao tiếp với Windows cho taskbar riêng: ẩn/hiện taskbar gốc (có sao lưu trạng thái),
//! giữ chỗ ở đáy màn hình (AppBar), danh sách cửa sổ, icon app, phím tắt hệ thống.

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

fn state_file() -> std::path::PathBuf {
    crate::config::dir().join("taskbar-original-state.txt")
}

/// Các cửa sổ taskbar THẬT của Windows (bỏ qua cửa sổ khay riêng của app, cùng tên lớp).
fn real_windows(class: PCWSTR) -> Vec<HWND> {
    let me = std::process::id();
    let mut out = Vec::new();
    unsafe {
        let mut prev = HWND::default();
        while let Ok(t) = FindWindowExW(HWND::default(), prev, class, PCWSTR::null()) {
            let mut pid = 0u32;
            GetWindowThreadProcessId(t, Some(&mut pid));
            if pid != me {
                out.push(t);
            }
            prev = t;
        }
    }
    out
}

/// Taskbar thật (màn hình chính) của Windows.
pub fn real_tray() -> Option<HWND> {
    real_windows(w!("Shell_TrayWnd")).into_iter().next()
}

pub fn tray_wnd() -> Option<HWND> {
    real_tray()
}

fn all_real_taskbars() -> Vec<HWND> {
    let mut v = real_windows(w!("Shell_TrayWnd"));
    v.extend(real_windows(w!("Shell_SecondaryTrayWnd")));
    v
}

fn appbar_data(hwnd: HWND) -> APPBARDATA {
    APPBARDATA { cbSize: std::mem::size_of::<APPBARDATA>() as u32, hWnd: hwnd, ..Default::default() }
}

fn get_state() -> u32 {
    let mut abd = appbar_data(HWND::default());
    unsafe { SHAppBarMessage(ABM_GETSTATE, &mut abd) as u32 }
}

fn set_state(state: u32) {
    if let Some(t) = tray_wnd() {
        let mut abd = appbar_data(t);
        abd.lParam = LPARAM(state as isize);
        unsafe {
            SHAppBarMessage(ABM_SETSTATE, &mut abd);
        }
    }
}

/// Các màn hình (vùng) đang dùng taskbar của app. Taskbar gốc chỉ bị ẩn trên các màn hình này.
static COVERED: std::sync::Mutex<Vec<(i32, i32, i32, i32)>> = std::sync::Mutex::new(Vec::new());

/// Báo danh sách màn hình đang dùng taskbar của app.
pub fn set_covered(mons: &[RECT]) {
    if let Ok(mut c) = COVERED.lock() {
        *c = mons.iter().map(|r| (r.left, r.top, r.right, r.bottom)).collect();
    }
}

/// Taskbar gốc `t` có nằm trên màn hình đang dùng taskbar của app không.
fn on_covered(t: HWND) -> bool {
    let cover = COVERED.lock().map(|c| c.clone()).unwrap_or_default();
    unsafe {
        let hm = MonitorFromWindow(t, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if !GetMonitorInfoW(hm, &mut mi).as_bool() {
            return false;
        }
        let r = mi.rcMonitor;
        cover.contains(&(r.left, r.top, r.right, r.bottom))
    }
}

fn original_state() -> Option<u32> {
    std::fs::read_to_string(state_file()).ok().and_then(|s| s.trim().parse::<u32>().ok())
}

/// Nhường chỗ cho taskbar của app, CHỈ trên các màn hình đang dùng nó (xem `set_covered`):
/// ẩn hẳn cửa sổ taskbar gốc ở đó (taskbar gốc bị ẩn thì không giữ chỗ nữa), màn hình còn lại
/// giữ taskbar Windows như cũ. Không bật "tự ẩn" của Windows: khi bật, Windows bỏ qua phần giữ
/// chỗ của taskbar app, cửa sổ phóng to sẽ đè lên thanh.
/// Trạng thái gốc được ghi ra file trước, để luôn trả lại được kể cả khi app bị lỗi.
pub fn hide_windows_taskbar() {
    let f = state_file();
    if !f.exists() {
        let _ = std::fs::create_dir_all(crate::config::dir());
        let _ = std::fs::write(&f, get_state().to_string());
    }
    let (hide, keep): (Vec<HWND>, Vec<HWND>) = all_real_taskbars().into_iter().partition(|t| on_covered(*t));
    let want = original_state().unwrap_or(get_state()) & ABS_AUTOHIDE;
    if get_state() & ABS_AUTOHIDE != want {
        set_state(want);
    }
    unsafe {
        for t in hide {
            if IsWindowVisible(t).as_bool() {
                let _ = ShowWindow(t, SW_HIDE);
            }
        }
        for t in keep {
            if !IsWindowVisible(t).as_bool() {
                let _ = ShowWindow(t, SW_SHOW);
            }
        }
    }
}

/// Đặt vùng làm việc của màn hình `mon` chừa đúng `reserve` px ở đáy cho taskbar app
/// (Windows đôi khi vẫn tính phần của taskbar gốc đã ẩn, hoặc bỏ qua taskbar app).
/// Explorer tự tính lại vùng làm việc khi taskbar gốc hiện lại, nên không cần trả lại riêng.
pub fn set_work_area(mon: RECT, reserve: i32) {
    let mut r = RECT { left: mon.left, top: mon.top, right: mon.right, bottom: mon.bottom - reserve };
    unsafe {
        let _ = SystemParametersInfoW(SPI_SETWORKAREA, 0, Some(&mut r as *mut RECT as *mut _), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0));
    }
}

/// Trả vùng làm việc của màn hình `mon` cho taskbar gốc (khi tắt taskbar app ở màn hình đó).
pub fn release_work_area(mon: RECT) {
    if get_state() & ABS_AUTOHIDE != 0 {
        set_work_area(mon, 0);
        return;
    }
    let mut bottom = mon.bottom;
    for t in all_real_taskbars() {
        let mut r = RECT::default();
        unsafe {
            if GetWindowRect(t, &mut r).is_ok() && r.left < mon.right && r.right > mon.left && r.top < mon.bottom && r.bottom > mon.top && r.bottom >= mon.bottom - 2 {
                bottom = bottom.min(r.top);
            }
        }
    }
    set_work_area(mon, mon.bottom - bottom);
}

/// Có taskbar gốc nào hiện ra trên màn hình đang dùng taskbar của app không.
pub fn windows_taskbar_visible() -> bool {
    all_real_taskbars().iter().any(|t| unsafe { IsWindowVisible(*t).as_bool() } && on_covered(*t))
}

/// Trả taskbar gốc về đúng như trước khi dùng app: trạng thái tự ẩn ban đầu, hiện ra,
/// màn hình phụ cũng hiện, và báo cho Windows cập nhật lại.
pub fn restore_windows_taskbar() {
    let f = state_file();
    let orig = std::fs::read_to_string(&f).ok().and_then(|s| s.trim().parse::<u32>().ok());
    if let Some(s) = orig {
        set_state(s);
    }
    unsafe {
        for t in all_real_taskbars() {
            let _ = ShowWindow(t, SW_SHOW);
        }
        // Kiểm tra lại một lần cho chắc.
        std::thread::sleep(std::time::Duration::from_millis(150));
        if let Some(t) = tray_wnd() {
            if !IsWindowVisible(t).as_bool() {
                let _ = ShowWindow(t, SW_SHOW);
            }
        }
        let msg = w!("TraySettings");
        let _ = SendMessageTimeoutW(HWND_BROADCAST, WM_SETTINGCHANGE, WPARAM(0), LPARAM(msg.as_ptr() as isize), SMTO_ABORTIFHUNG, 1000, None);
    }
    let _ = std::fs::remove_file(f);
    crate::log(&format!("đã trả taskbar gốc: hiện={}, tự ẩn={}", windows_taskbar_visible(), get_state() & ABS_AUTOHIDE != 0));
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Net {
    Wifi,
    Ethernet,
    Offline,
}

/// Đang kết nối mạng bằng Wi-Fi hay dây (Ethernet)? Dựa vào card mạng đang bật và có cổng ra Internet.
pub fn network() -> Net {
    use windows::Win32::NetworkManagement::IpHelper::{GetAdaptersAddresses, GAA_FLAG_INCLUDE_GATEWAYS, IP_ADAPTER_ADDRESSES_LH};
    use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    unsafe {
        let mut size = 16 * 1024u32;
        let mut buf: Vec<u8> = vec![0; size as usize];
        let mut r = GetAdaptersAddresses(0, GAA_FLAG_INCLUDE_GATEWAYS, None, Some(buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH), &mut size);
        if r == 111 {
            buf = vec![0; size as usize];
            r = GetAdaptersAddresses(0, GAA_FLAG_INCLUDE_GATEWAYS, None, Some(buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH), &mut size);
        }
        if r != 0 {
            return Net::Offline;
        }
        let (mut wifi, mut wired) = (false, false);
        let mut p = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
        while !p.is_null() {
            let a = &*p;
            if a.OperStatus == IfOperStatusUp && !a.FirstGatewayAddress.is_null() {
                match a.IfType {
                    71 => wifi = true,
                    6 => wired = true,
                    _ => {}
                }
            }
            p = a.Next;
        }
        if wired {
            Net::Ethernet
        } else if wifi {
            Net::Wifi
        } else {
            Net::Offline
        }
    }
}

/// Lần trước app có ẩn taskbar mà chưa trả lại (ví dụ bị tắt đột ngột)?
pub fn needs_recovery() -> bool {
    state_file().exists()
}

/// Hiện taskbar gốc tạm thời (để dùng các icon ở khay hệ thống): tắt tự ẩn một lúc.
#[allow(dead_code)]
pub fn peek_windows_taskbar(show: bool) {
    if show {
        set_state(get_state() & !ABS_AUTOHIDE);
        crate::log(&format!("hiện tạm taskbar gốc, thấy được: {}", windows_taskbar_visible()));
    } else {
        hide_windows_taskbar();
    }
}

/// Tiến trình canh: chờ app chính tắt (kể cả bị tắt đột ngột), rồi trả taskbar gốc nếu còn ẩn.
pub fn watchdog(pid: u32) {
    use windows::Win32::System::Threading::{OpenProcess, WaitForSingleObject, INFINITE, PROCESS_SYNCHRONIZE};
    unsafe {
        if let Ok(h) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            WaitForSingleObject(h, INFINITE);
            let _ = CloseHandle(h);
        }
    }
    if needs_recovery() {
        restore_windows_taskbar();
        crate::log("tiến trình canh: app đã tắt đột ngột, đã trả lại taskbar gốc");
    }
}

/// Chạy tiến trình canh cho app hiện tại.
pub fn spawn_watchdog() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).arg("--watchdog").arg(std::process::id().to_string()).spawn();
    }
}

/// Giữ một dải ở đáy màn hình chính cho taskbar của app (cửa sổ phóng to sẽ không đè lên).
pub fn appbar_register(hwnd: HWND, monitor: RECT, reserve: i32) {
    let mut abd = appbar_data(hwnd);
    abd.uCallbackMessage = WM_APP + 7;
    unsafe {
        SHAppBarMessage(ABM_NEW, &mut abd);
    }
    appbar_set_pos(hwnd, monitor, reserve);
}

pub fn appbar_set_pos(hwnd: HWND, monitor: RECT, reserve: i32) {
    let mut abd = appbar_data(hwnd);
    abd.uEdge = ABE_BOTTOM;
    abd.rc = RECT { left: monitor.left, top: monitor.bottom - reserve, right: monitor.right, bottom: monitor.bottom };
    unsafe {
        SHAppBarMessage(ABM_QUERYPOS, &mut abd);
        abd.rc.top = abd.rc.bottom - reserve;
        SHAppBarMessage(ABM_SETPOS, &mut abd);
    }
}

/// Đáy vùng làm việc (phần không bị taskbar chiếm) của màn hình `mon`.
pub fn work_bottom(mon: RECT) -> Option<i32> {
    unsafe {
        let hm = MonitorFromPoint(POINT { x: (mon.left + mon.right) / 2, y: (mon.top + mon.bottom) / 2 }, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        GetMonitorInfoW(hm, &mut mi).as_bool().then_some(mi.rcWork.bottom)
    }
}

pub fn appbar_remove(hwnd: HWND) {
    let mut abd = appbar_data(hwnd);
    unsafe {
        SHAppBarMessage(ABM_REMOVE, &mut abd);
    }
}

/// Có app/game toàn màn hình hoặc đang trình chiếu không (khi đó taskbar phải tránh ra).
pub fn fullscreen_active(mon: RECT) -> bool {
    // Xét cửa sổ nằm trên cùng của CHÍNH màn hình này (theo thứ tự chồng lớp), không xét cửa sổ
    // đang được chọn: bấm sang màn bên kia thì video toàn màn hình ở đây vẫn che taskbar,
    // còn YouTube toàn màn hình ở màn bên kia không làm taskbar ở đây biến mất.
    unsafe {
        let me = std::process::id();
        let mut h = GetTopWindow(HWND::default()).unwrap_or_default();
        while !h.is_invalid() {
            let next = GetWindow(h, GW_HWNDNEXT).unwrap_or_default();
            if let Some(full) = covers_monitor(h, mon, me) {
                *LAST_TOP.lock().unwrap_or_else(|e| e.into_inner()) = format!("{} {:#x}", class_name(h), h.0 as usize);
                return full;
            }
            h = next;
        }
        LAST_TOP.lock().unwrap_or_else(|e| e.into_inner()).clear();
        false
    }
}

/// Tiến trình sở hữu cửa sổ `h`.
pub fn window_pid(h: HWND) -> u32 {
    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(h, Some(&mut pid));
    }
    pid
}

/// Sau khi chuột phải vào icon khay: app đang mở menu, đã mở cửa sổ, hay chưa có gì.
pub enum TrayMenu {
    Menu,
    Window,
    Nothing,
}

/// Xét phản hồi của app (tiến trình `pid`) sau cú chuột phải tại điểm `pt` (pixel màn hình).
/// Menu: menu chuẩn của Windows (#32768) đang hiện, hoặc cửa sổ popup không thanh tiêu đề của app
/// nằm gần chỗ bấm (menu tự vẽ của Qt, Electron…). Window: cửa sổ đang được chọn là cửa sổ có
/// thanh tiêu đề của app.
pub fn tray_menu_state(pid: u32, pt: (i32, i32), fg_before: isize) -> TrayMenu {
    unsafe {
        let mut h = GetTopWindow(HWND::default()).unwrap_or_default();
        let mut n = 0;
        while !h.is_invalid() && n < 400 {
            n += 1;
            let next = GetWindow(h, GW_HWNDNEXT).unwrap_or_default();
            if IsWindowVisible(h).as_bool() {
                if class_name(h) == "#32768" {
                    return TrayMenu::Menu;
                }
                if pid != 0 && window_pid(h) == pid {
                    let style = GetWindowLongW(h, GWL_STYLE) as u32;
                    let mut r = RECT::default();
                    let popup = style & WS_POPUP.0 != 0 && style & WS_CAPTION.0 != WS_CAPTION.0;
                    if popup && GetWindowRect(h, &mut r).is_ok() {
                        let (w, hh) = (r.right - r.left, r.bottom - r.top);
                        let near = pt.0 >= r.left - 80 && pt.0 <= r.right + 80 && pt.1 >= r.top - 80 && pt.1 <= r.bottom + 80;
                        if w > 20 && hh > 20 && w < 900 && hh < 1200 && near {
                            return TrayMenu::Menu;
                        }
                    }
                }
            }
            h = next;
        }
        let fg = GetForegroundWindow();
        // Chỉ tính cửa sổ vừa được đưa lên sau cú bấm (không phải cửa sổ app đang chọn từ trước)
        if pid != 0 && !fg.is_invalid() && fg.0 as isize != fg_before && window_pid(fg) == pid {
            let style = GetWindowLongW(fg, GWL_STYLE) as u32;
            if style & WS_CAPTION.0 == WS_CAPTION.0 {
                return TrayMenu::Window;
            }
        }
        TrayMenu::Nothing
    }
}

static LAST_TOP: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// Cửa sổ quyết định lần xét toàn màn hình gần nhất (lớp và handle), để ghi log.
pub fn last_fullscreen_reason() -> String {
    LAST_TOP.lock().map(|s| s.clone()).unwrap_or_default()
}

/// Cửa sổ `h` có đáng xét trên màn hình `mon` không: None = bỏ qua (ẩn, thu nhỏ, của app này,
/// cửa sổ công cụ, nằm ở màn khác, cửa sổ nổi nhỏ…); Some(true) = phủ kín màn hình kiểu
/// toàn màn hình; Some(false) = cửa sổ thường đang nằm trên cùng, hoặc đã tới desktop.
unsafe fn covers_monitor(h: HWND, mon: RECT, me: u32) -> Option<bool> {
    if !IsWindowVisible(h).as_bool() || IsIconic(h).as_bool() {
        return None;
    }
    let class = class_name(h);
    if matches!(class.as_str(), "Progman" | "WorkerW") {
        return Some(false);
    }
    if matches!(class.as_str(), "Shell_TrayWnd" | "Shell_SecondaryTrayWnd") {
        return None;
    }
    let mut pid = 0u32;
    GetWindowThreadProcessId(h, Some(&mut pid));
    if pid == me {
        return None;
    }
    let ex = GetWindowLongW(h, GWL_EXSTYLE) as u32;
    // Lớp phủ không nhận chuột/không nhận focus (overlay của driver, game launcher…)
    if ex & (WS_EX_TOOLWINDOW.0 | WS_EX_TRANSPARENT.0 | WS_EX_NOACTIVATE.0) != 0 {
        return None;
    }
    let mut cloaked = 0u32;
    let _ = DwmGetWindowAttribute(h, DWMWA_CLOAKED, &mut cloaked as *mut u32 as *mut _, 4);
    if cloaked != 0 {
        return None;
    }
    // Khung nhìn thấy của cửa sổ (không tính viền kéo giãn vô hình ~7px: cửa sổ phóng to ở màn
    // bên cạnh lấn sang màn này đúng phần viền đó).
    let mut r = RECT::default();
    if DwmGetWindowAttribute(h, DWMWA_EXTENDED_FRAME_BOUNDS, &mut r as *mut RECT as *mut _, std::mem::size_of::<RECT>() as u32).is_err() {
        if GetWindowRect(h, &mut r).is_err() {
            return None;
        }
    }
    if r.right <= r.left || r.bottom <= r.top {
        return None;
    }
    // Chỉ xét cửa sổ thuộc về màn hình này (màn chứa phần lớn cửa sổ)
    let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    if !GetMonitorInfoW(MonitorFromRect(&r, MONITOR_DEFAULTTONULL), &mut mi).as_bool() || mi.rcMonitor != mon {
        return None;
    }
    let covers = r.left <= mon.left && r.top <= mon.top && r.right >= mon.right && r.bottom >= mon.bottom;
    if !covers && ex & WS_EX_TOPMOST.0 != 0 {
        // Cửa sổ nổi nhỏ luôn ở trên (ảnh trong ảnh, thông báo…): xem tiếp cửa sổ bên dưới.
        return None;
    }
    // Cửa sổ phóng to bình thường còn thanh tiêu đề thì không tính
    // (Chrome/YouTube toàn màn hình vẫn bị Windows coi là "phóng to" nhưng đã bỏ thanh tiêu đề).
    let caption = (GetWindowLongW(h, GWL_STYLE) as u32 & WS_CAPTION.0) == WS_CAPTION.0;
    Some(covers && !(IsZoomed(h).as_bool() && caption))
}

fn class_name(h: HWND) -> String {
    let mut buf = [0u16; 128];
    let n = unsafe { GetClassNameW(h, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

pub struct AppWin {
    pub hwnd: isize,
    pub title: String,
}

unsafe extern "system" fn enum_cb(h: HWND, lp: LPARAM) -> BOOL {
    let v = &mut *(lp.0 as *mut Vec<AppWin>);
    if !IsWindowVisible(h).as_bool() {
        return TRUE;
    }
    if GetWindow(h, GW_OWNER).map(|o| !o.is_invalid()).unwrap_or(false) {
        return TRUE;
    }
    let ex = GetWindowLongPtrW(h, GWL_EXSTYLE) as u32;
    if ex & WS_EX_TOOLWINDOW.0 != 0 {
        return TRUE;
    }
    let mut cloaked = 0u32;
    let _ = DwmGetWindowAttribute(h, DWMWA_CLOAKED, &mut cloaked as *mut u32 as *mut _, 4);
    if cloaked != 0 {
        return TRUE;
    }
    let n = GetWindowTextLengthW(h);
    if n == 0 {
        return TRUE;
    }
    if matches!(class_name(h).as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" | "Windows.UI.Core.CoreWindow") {
        return TRUE;
    }
    let mut buf = vec![0u16; n as usize + 1];
    let len = GetWindowTextW(h, &mut buf);
    v.push(AppWin { hwnd: h.0 as isize, title: String::from_utf16_lossy(&buf[..len.max(0) as usize]) });
    TRUE
}

/// Các cửa sổ ứng dụng đang mở (giống danh sách Alt+Tab).
pub fn list_windows() -> Vec<AppWin> {
    let mut v: Vec<AppWin> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(enum_cb), LPARAM(&mut v as *mut Vec<AppWin> as isize));
    }
    v
}

pub fn foreground() -> isize {
    unsafe { GetForegroundWindow().0 as isize }
}

/// Bấm vào ô app: đang ở trên cùng thì thu nhỏ, không thì đưa lên.
pub fn activate(hwnd: isize) {
    let h = HWND(hwnd as *mut _);
    unsafe {
        if GetForegroundWindow() == h && !IsIconic(h).as_bool() {
            let _ = ShowWindow(h, SW_MINIMIZE);
        } else {
            if IsIconic(h).as_bool() {
                let _ = ShowWindow(h, SW_RESTORE);
            }
            let _ = SetForegroundWindow(h);
        }
    }
}

/// Đưa cửa sổ lên trước (không thu nhỏ nếu nó đã đang được chọn).
pub fn bring(hwnd: isize) {
    let h = HWND(hwnd as *mut _);
    unsafe {
        if IsIconic(h).as_bool() {
            let _ = ShowWindow(h, SW_RESTORE);
        }
        let _ = SetForegroundWindow(h);
    }
}

/// Nhấn tổ hợp phím (ví dụ Win để mở Start, Win+A để mở cài đặt nhanh).
pub fn press(keys: &[VIRTUAL_KEY]) {
    let mk = |vk: VIRTUAL_KEY, up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) }, time: 0, dwExtraInfo: 0 },
        },
    };
    let mut inputs: Vec<INPUT> = keys.iter().map(|&k| mk(k, false)).collect();
    inputs.extend(keys.iter().rev().map(|&k| mk(k, true)));
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

pub const KEY_WIN: VIRTUAL_KEY = VK_LWIN;
pub const KEY_A: VIRTUAL_KEY = VIRTUAL_KEY(0x41);
pub const KEY_X: VIRTUAL_KEY = VIRTUAL_KEY(0x58);
#[allow(dead_code)]
pub const KEY_N: VIRTUAL_KEY = VIRTUAL_KEY(0x4E);

/// Tên file chương trình (chữ thường) của cửa sổ.
pub fn exe_name(hwnd: isize) -> String {
    exe_of(HWND(hwnd as *mut _)).and_then(|p| p.rsplit(['/', std::path::MAIN_SEPARATOR]).next().map(|f| f.to_lowercase())).unwrap_or_default()
}

/// Ảnh RGBA (đã nhân alpha) của một HICON.
pub fn icon_rgba(icon: HICON, size: i32) -> Option<Vec<u8>> {
    icon_pixels(icon, size)
}

/// Đường dẫn file chương trình của cửa sổ.
fn exe_of(h: HWND) -> Option<String> {
    use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(h, Some(&mut pid));
        let hp = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let r = QueryFullProcessImageNameW(hp, PROCESS_NAME_WIN32, windows::core::PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(hp);
        r.ok()?;
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

/// Icon đúng kích thước lấy thẳng từ file chương trình (nét hơn icon của cửa sổ).
fn exe_icon(h: HWND, size: i32) -> Option<HICON> {
    let path = exe_of(h)?;
    if path.to_lowercase().ends_with("applicationframehost.exe") {
        return None; // app UWP: dùng icon của cửa sổ
    }
    let mut name = [0u16; 260];
    for (i, c) in path.encode_utf16().take(259).enumerate() {
        name[i] = c;
    }
    let mut icons = [HICON::default()];
    let n = unsafe { PrivateExtractIconsW(&name, 0, size, size, Some(&mut icons), None, 0) };
    if n == 0 || n == u32::MAX || icons[0].is_invalid() {
        return None;
    }
    Some(icons[0])
}

/// Icon của cửa sổ, trả về RGBA đã nhân alpha (size x size).
fn window_icon(hwnd: isize, size: i32) -> Option<Vec<u8>> {
    let h = HWND(hwnd as *mut _);
    unsafe {
        let mut hicon: usize = 0;
        for kind in [ICON_BIG, ICON_SMALL2, ICON_SMALL] {
            let mut res = 0usize;
            let delivered = SendMessageTimeoutW(h, WM_GETICON, WPARAM(kind as usize), LPARAM(0), SMTO_ABORTIFHUNG, 40, Some(&mut res));
            if delivered.0 == 0 {
                break;
            }
            if res != 0 {
                hicon = res;
                break;
            }
        }
        if hicon == 0 {
            hicon = GetClassLongPtrW(h, GCLP_HICON);
        }
        if hicon == 0 {
            hicon = GetClassLongPtrW(h, GCLP_HICONSM);
        }
        if hicon != 0 {
            if let Some(pixels) = icon_pixels(HICON(hicon as *mut _), size) {
                return Some(pixels);
            }
        }
    }
    if let Some(icon) = exe_icon(h, size) {
        let out = icon_pixels(icon, size);
        unsafe {
            let _ = DestroyIcon(icon);
        }
        if out.is_some() {
            return out;
        }
    }
    None
}

fn icon_pixels(icon: HICON, size: i32) -> Option<Vec<u8>> {
    unsafe {
        // Vẽ icon lên nền đen và nền trắng, so sánh để tách được độ trong suốt.
        let black = draw_icon(icon, size, 0x00)?;
        let white = draw_icon(icon, size, 0xFF)?;
        let mut out = vec![0u8; (size * size * 4) as usize];
        for i in 0..(size * size) as usize {
            let (b, wh) = (&black[i * 4..i * 4 + 4], &white[i * 4..i * 4 + 4]);
            let diff = (0..3).map(|c| wh[c] as i32 - b[c] as i32).max().unwrap_or(0).clamp(0, 255);
            let a = (255 - diff) as u8;
            // black là BGRA, đã là màu * alpha
            out[i * 4] = b[2].min(a);
            out[i * 4 + 1] = b[1].min(a);
            out[i * 4 + 2] = b[0].min(a);
            out[i * 4 + 3] = a;
        }
        out.chunks_exact(4).any(|p| p[3] != 0).then_some(out)
    }
}

unsafe fn draw_icon(icon: HICON, size: i32, bg: u8) -> Option<Vec<u8>> {
    let screen = GetDC(HWND::default());
    let dc = CreateCompatibleDC(screen);
    let mut bmi = BITMAPINFO::default();
    bmi.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: size,
        biHeight: -size,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };
    let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
    let bmp = CreateDIBSection(dc, &bmi, DIB_RGB_COLORS, &mut bits, HANDLE::default(), 0).ok();
    let mut result = None;
    if let Some(bmp) = bmp {
        let old = SelectObject(dc, bmp);
        let n = (size * size * 4) as usize;
        std::ptr::write_bytes(bits as *mut u8, bg, n);
        if DrawIconEx(dc, 0, 0, icon, size, size, 0, HBRUSH::default(), DI_NORMAL).is_ok() {
            result = Some(std::slice::from_raw_parts(bits as *const u8, n).to_vec());
        }
        SelectObject(dc, old);
        let _ = DeleteObject(bmp);
    }
    let _ = DeleteDC(dc);
    ReleaseDC(HWND::default(), screen);
    result
}

/// Bộ đệm cho cửa sổ trong suốt từng điểm ảnh (UpdateLayeredWindow).
pub struct Layered {
    dc: HDC,
    bmp: HBITMAP,
    old: HGDIOBJ,
    bits: *mut u8,
    pub w: i32,
    pub h: i32,
    warned: bool,
    /// Độ rõ của cả cửa sổ (hiệu ứng hiện dần) và độ lệch dọc (hiệu ứng trượt)
    pub alpha: u8,
    pub dy: i32,
    last: Option<(isize, i32, i32)>,
}

impl Layered {
    pub fn new(w: i32, h: i32) -> Option<Self> {
        unsafe {
            let screen = GetDC(HWND::default());
            let dc = CreateCompatibleDC(screen);
            ReleaseDC(HWND::default(), screen);
            let mut bmi = BITMAPINFO::default();
            bmi.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            };
            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let bmp = CreateDIBSection(dc, &bmi, DIB_RGB_COLORS, &mut bits, HANDLE::default(), 0).ok()?;
            let old = SelectObject(dc, bmp);
            Some(Self { dc, bmp, old, bits: bits as *mut u8, w, h, warned: false, alpha: 255, dy: 0, last: None })
        }
    }

    /// Đẩy ảnh RGBA (đã nhân alpha) lên cửa sổ tại vị trí (x, y) trên màn hình.
    /// Nếu cửa sổ bị mất kiểu "trong suốt" (do ai đó đặt lại kiểu), tự đặt lại rồi vẽ lại.
    pub fn present(&mut self, hwnd: HWND, x: i32, y: i32, rgba: &[u8]) {
        if !self.present_once(hwnd, x, y, rgba) {
            crate::log("cập nhật ảnh trong suốt lỗi: đặt lại kiểu cửa sổ rồi vẽ lại");
            make_bar_window(hwnd, x, y, self.w, self.h);
            unsafe {
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            }
            if self.present_once(hwnd, x, y, rgba) {
                crate::log("đã khôi phục kiểu cửa sổ trong suốt");
            }
        }
    }

    fn present_once(&mut self, hwnd: HWND, x: i32, y: i32, rgba: &[u8]) -> bool {
        let n = (self.w * self.h * 4) as usize;
        if rgba.len() < n {
            return true;
        }
        unsafe {
            let dst = std::slice::from_raw_parts_mut(self.bits, n);
            for (d, s) in dst.chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
                d[0] = s[2];
                d[1] = s[1];
                d[2] = s[0];
                d[3] = s[3];
            }
            self.last = Some((hwnd.0 as isize, x, y));
            let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: self.alpha, AlphaFormat: AC_SRC_ALPHA as u8 };
            let pt = POINT { x, y: y + self.dy };
            let size = SIZE { cx: self.w, cy: self.h };
            let src = POINT { x: 0, y: 0 };
            let screen = GetDC(HWND::default());
            let r = UpdateLayeredWindow(hwnd, screen, Some(&pt), Some(&size), self.dc, Some(&src), COLORREF(0), Some(&blend), ULW_ALPHA);
            ReleaseDC(HWND::default(), screen);
            if let Err(e) = &r {
                if !self.warned {
                    self.warned = true;
                    crate::log(&format!("UpdateLayeredWindow lỗi ({}x{} tại {x},{y}): {e}", self.w, self.h));
                }
            }
            r.is_ok()
        }
    }
}

impl Layered {
    /// Chỉ cập nhật một vùng nhỏ (x0, y0, x1, y1 – pixel) của ảnh: đổi màu vùng đó rồi báo Windows
    /// chỉ vẽ lại vùng đó. Rẻ hơn nhiều so với đẩy cả thanh mỗi khung hiệu ứng.
    pub fn present_rect(&mut self, hwnd: HWND, x: i32, y: i32, rgba: &[u8], r: (i32, i32, i32, i32)) {
        let (w, h) = (self.w, self.h);
        let (x0, y0, x1, y1) = (r.0.clamp(0, w), r.1.clamp(0, h), r.2.clamp(0, w), r.3.clamp(0, h));
        if x0 >= x1 || y0 >= y1 || rgba.len() < (w * h * 4) as usize || self.last.map(|l| l.0) != Some(hwnd.0 as isize) {
            self.present(hwnd, x, y, rgba);
            return;
        }
        unsafe {
            let dst = std::slice::from_raw_parts_mut(self.bits, (w * h * 4) as usize);
            for row in y0..y1 {
                let a = ((row * w + x0) * 4) as usize;
                let b = ((row * w + x1) * 4) as usize;
                for (d, s) in dst[a..b].chunks_exact_mut(4).zip(rgba[a..b].chunks_exact(4)) {
                    d[0] = s[2];
                    d[1] = s[1];
                    d[2] = s[0];
                    d[3] = s[3];
                }
            }
            let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: self.alpha, AlphaFormat: AC_SRC_ALPHA as u8 };
            let pt = POINT { x, y: y + self.dy };
            let size = SIZE { cx: w, cy: h };
            let src = POINT { x: 0, y: 0 };
            let dirty = RECT { left: x0, top: y0, right: x1, bottom: y1 };
            let screen = GetDC(HWND::default());
            let info = UPDATELAYEREDWINDOWINFO {
                cbSize: std::mem::size_of::<UPDATELAYEREDWINDOWINFO>() as u32,
                hdcDst: screen,
                pptDst: &pt,
                psize: &size,
                hdcSrc: self.dc,
                pptSrc: &src,
                crKey: COLORREF(0),
                pblend: &blend,
                dwFlags: ULW_ALPHA,
                prcDirty: &dirty,
            };
            let ok = UpdateLayeredWindowIndirect(hwnd, &info).as_bool();
            ReleaseDC(HWND::default(), screen);
            if !ok {
                self.present(hwnd, x, y, rgba);
            }
        }
    }

    /// Đẩy lại ảnh đã có với độ rõ/độ lệch mới (dùng cho hiệu ứng, không vẽ lại).
    pub fn reshow(&mut self) {
        let Some((h, x, y)) = self.last else { return };
        unsafe {
            let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: self.alpha, AlphaFormat: AC_SRC_ALPHA as u8 };
            let pt = POINT { x, y: y + self.dy };
            let size = SIZE { cx: self.w, cy: self.h };
            let src = POINT { x: 0, y: 0 };
            let screen = GetDC(HWND::default());
            let _ = UpdateLayeredWindow(HWND(h as *mut _), screen, Some(&pt), Some(&size), self.dc, Some(&src), COLORREF(0), Some(&blend), ULW_ALPHA);
            ReleaseDC(HWND::default(), screen);
        }
    }
}

/// Hiệu ứng mở/đóng của cửa sổ nổi như Windows 11: mở thì trượt lên + hiện dần,
/// đóng thì trượt xuống + mờ đi rồi ẩn hẳn.
#[derive(Clone, Copy)]
pub struct Fade {
    pub start: std::time::Instant,
    pub closing: bool,
}

pub const FADE_SLIDE: f32 = 14.0;

impl Fade {
    pub fn open() -> Option<Fade> {
        Some(Fade { start: std::time::Instant::now(), closing: false })
    }
    pub fn close() -> Option<Fade> {
        Some(Fade { start: std::time::Instant::now(), closing: true })
    }
}

/// (độ rõ 0..1, độ lệch 0..1, đã xong) của hiệu ứng ở thời điểm hiện tại.
pub fn fade_state(f: &Fade) -> (f32, f32, bool) {
    let dur = if f.closing { 130.0 } else { 180.0 };
    let p = (f.start.elapsed().as_secs_f32() * 1000.0 / dur).min(1.0);
    if f.closing {
        let e = p * p;
        (1.0 - e, e, p >= 1.0)
    } else {
        let e = 1.0 - (1.0 - p).powi(3);
        (e, 1.0 - e, p >= 1.0)
    }
}

/// Một bước hiệu ứng cho cửa sổ trong suốt (layered). Trả về true nếu còn chạy.
pub fn fade_step(fade: &mut Option<Fade>, l: &mut Option<Layered>, hwnd: HWND, k: f32) -> bool {
    let Some(f) = *fade else { return false };
    let (a, off, done) = fade_state(&f);
    if let Some(l) = l.as_mut() {
        l.alpha = (a * 255.0).round() as u8;
        l.dy = (off * FADE_SLIDE * k).round() as i32;
        l.reshow();
    }
    if done {
        *fade = None;
        if f.closing {
            show(hwnd, false);
            if let Some(l) = l.as_mut() {
                l.alpha = 255;
                l.dy = 0;
            }
        }
        return false;
    }
    true
}

/// Thứ tự icon khay mà Windows lưu (UIOrderList): mỗi mục là GUID của icon (nếu có)
/// hoặc (tên chương trình, UID).
fn read_tray_order() -> Vec<(Option<String>, String, u32)> {
    let Some(b) = reg_get(r"Control Panel\NotifyIconSettings", "UIOrderList") else { return Vec::new() };
    let mut out = Vec::new();
    for c in b.chunks_exact(8) {
        let id = u64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]);
        let sub = format!(r"Control Panel\NotifyIconSettings\{id}");
        let guid = reg_get(&sub, "IconGuid").map(|v| utf16_bytes(&v).to_uppercase());
        let exe = reg_get(&sub, "ExecutablePath").map(|v| utf16_bytes(&v)).unwrap_or_default();
        let file = exe.rsplit(['\\', '/']).next().unwrap_or("").to_lowercase();
        let uid = reg_dword(&sub, "UID").unwrap_or(0);
        out.push((guid, file, uid));
    }
    out
}

/// Icon khay mà Windows đang có nhưng không gửi sang taskbar của app (thường là app chạy
/// quyền hệ thống như NVIDIA, đăng ký với Explorer trước khi app khởi động và không đăng ký lại).
/// Lấy từ dữ liệu Windows lưu (NotifyIconSettings): chỉ những app đang chạy, có mã icon và
/// chú thích (dấu hiệu icon khay thật), chưa có trong danh sách đã nhận.
/// Trả về (tên chương trình, UID, chú thích, ảnh 32x32 RGBA đã nhân alpha, được ghim ra ngoài?).
fn scan_tray_proxies() -> Vec<(String, u32, String, Vec<u8>, bool)> {
    use windows::Win32::System::Registry::*;
    let running = running_exes();
    let mut out: Vec<(String, u32, String, Vec<u8>, bool)> = Vec::new();
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, w!(r"Control Panel\NotifyIconSettings"), 0, KEY_READ, &mut key).is_err() {
            return out;
        }
        let mut i = 0u32;
        loop {
            let mut name = [0u16; 256];
            let mut len = name.len() as u32;
            if RegEnumKeyExW(key, i, windows::core::PWSTR(name.as_mut_ptr()), &mut len, None, windows::core::PWSTR::null(), None, None).is_err() {
                break;
            }
            i += 1;
            let sub = format!(r"Control Panel\NotifyIconSettings\{}", String::from_utf16_lossy(&name[..len as usize]));
            let Some(uid) = reg_dword(&sub, "UID") else { continue };
            let tip = reg_get(&sub, "InitialTooltip").map(|b| utf16_bytes(&b)).unwrap_or_default();
            if tip.is_empty() || reg_get(&sub, "IconGuid").is_some() {
                continue;
            }
            let exe = reg_get(&sub, "ExecutablePath").map(|b| utf16_bytes(&b)).unwrap_or_default();
            let file = exe.rsplit(['\\', '/']).next().unwrap_or("").to_lowercase();
            // Không bù icon của chính app: app tự ẩn icon khi cửa sổ cài đặt đang mở
            if file.is_empty() || file == "season-desktop.exe" || !running.contains(&file) || out.iter().any(|o| o.0 == file) {
                continue;
            }
            let Some(png) = reg_get(&sub, "IconSnapshot") else { continue };
            let Ok(src) = tiny_skia::Pixmap::decode_png(&png) else { continue };
            let Some(mut pm) = tiny_skia::Pixmap::new(32, 32) else { continue };
            let s = 32.0 / src.width().max(src.height()).max(1) as f32;
            pm.draw_pixmap(0, 0, src.as_ref(), &tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() }, tiny_skia::Transform::from_scale(s, s), None);
            let promoted = reg_dword(&sub, "IsPromoted") == Some(1);
            out.push((file, uid, tip, pm.data().to_vec(), promoted));
        }
        let _ = RegCloseKey(key);
    }
    out
}

type Proxy = (String, u32, String, Vec<u8>, bool);

/// Thứ tự khay và icon thay thế đọc sẵn ở luồng nền (đọc registry, liệt kê tiến trình và giải mã
/// ảnh tốn 7–10 ms: làm trên luồng vẽ thì hình nền, taskbar bị giật). Làm mới mỗi 3 giây.
static TRAY_CACHE: std::sync::Mutex<Option<(Vec<(Option<String>, String, u32)>, Vec<Proxy>)>> = std::sync::Mutex::new(None);

fn tray_cache() -> (Vec<(Option<String>, String, u32)>, Vec<Proxy>) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let fill = || {
            let v = (read_tray_order(), scan_tray_proxies());
            if let Ok(mut c) = TRAY_CACHE.lock() {
                *c = Some(v);
            }
        };
        fill();
        let _ = std::thread::Builder::new().name("khay".into()).spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(3));
            let v = (read_tray_order(), scan_tray_proxies());
            if let Ok(mut c) = TRAY_CACHE.lock() {
                *c = Some(v);
            }
        });
    });
    TRAY_CACHE.lock().ok().and_then(|c| c.clone()).unwrap_or_default()
}

/// Thứ tự icon khay như Windows lưu (đọc sẵn ở luồng nền).
pub fn tray_order() -> Vec<(Option<String>, String, u32)> {
    tray_cache().0
}

/// Icon Windows có nhưng app không gửi sang khay của app (đọc sẵn ở luồng nền), trừ những app đã gửi.
pub fn tray_proxies(received: &[(String, u32)]) -> Vec<Proxy> {
    tray_cache().1.into_iter().filter(|p| !received.iter().any(|(e, _)| *e == p.0)).collect()
}

/// Chuỗi GUID dạng {XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX} từ 16 byte (kiểu GUID của Windows).
pub fn guid_string(g: &[u8; 16]) -> String {
    format!(
        "{{{:02X}{:02X}{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        g[3], g[2], g[1], g[0], g[5], g[4], g[7], g[6], g[8], g[9], g[10], g[11], g[12], g[13], g[14], g[15]
    )
}

/// Theo dõi đổi cửa sổ đang chọn (để thanh cập nhật ngay như taskbar gốc).
static FG_CHANGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// Có cửa sổ vừa đổi trạng thái (chọn, thu nhỏ, phóng to, di chuyển): hình nền xét lại ngay.
static WIN_CHANGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

unsafe extern "system" fn fg_hook(_: windows::Win32::UI::Accessibility::HWINEVENTHOOK, _: u32, _: HWND, _: i32, _: i32, _: u32, _: u32) {
    FG_CHANGED.store(true, std::sync::atomic::Ordering::Relaxed);
    WIN_CHANGED.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// Có cửa sổ vừa hiện/ẩn (mở/đóng app): danh sách app trên thanh cập nhật ngay.
static APPS_CHANGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

unsafe extern "system" fn loc_hook(_: windows::Win32::UI::Accessibility::HWINEVENTHOOK, ev: u32, h: HWND, obj: i32, child: i32, _: u32, _: u32) {
    // Chỉ cửa sổ chính (không phải con trỏ chuột, thanh cuộn...)
    if obj == 0 && child == 0 && !h.is_invalid() && GetAncestor(h, GA_ROOT) == h {
        WIN_CHANGED.store(true, std::sync::atomic::Ordering::Relaxed);
        if ev == EVENT_OBJECT_SHOW || ev == EVENT_OBJECT_HIDE || ev == EVENT_OBJECT_DESTROY {
            APPS_CHANGED.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

/// Danh sách ghim của Windows vừa đổi (nghe thẳng khoá registry Taskband, không phải hỏi lại định kỳ).
static PINS_CHANGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Ghim / bỏ ghim / đổi chỗ trên taskbar Windows vừa xảy ra (đọc rồi xoá cờ).
pub fn pins_changed() -> bool {
    use windows::Win32::System::Registry::*;
    use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject, INFINITE};
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let _ = std::thread::Builder::new().name("nghe-ghim".into()).spawn(|| unsafe {
            let mut key = HKEY::default();
            if RegOpenKeyExW(HKEY_CURRENT_USER, w!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Taskband"), 0, KEY_NOTIFY, &mut key).is_err() {
                return;
            }
            let Ok(ev) = CreateEventW(None, false, false, PCWSTR::null()) else { return };
            loop {
                if RegNotifyChangeKeyValue(key, false, REG_NOTIFY_CHANGE_LAST_SET, ev, true).is_err() {
                    return;
                }
                WaitForSingleObject(ev, INFINITE);
                PINS_CHANGED.store(true, std::sync::atomic::Ordering::Relaxed);
                APPS_CHANGED.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        });
    });
    PINS_CHANGED.swap(false, std::sync::atomic::Ordering::Relaxed)
}

/// Có cửa sổ vừa mở/đóng (đọc rồi xoá cờ).
pub fn apps_changed() -> bool {
    APPS_CHANGED.swap(false, std::sync::atomic::Ordering::Relaxed)
}

/// Có cửa sổ vừa đổi trạng thái (đọc rồi xoá cờ).
pub fn windows_changed() -> bool {
    WIN_CHANGED.swap(false, std::sync::atomic::Ordering::Relaxed)
}

pub fn watch_foreground() {
    use windows::Win32::UI::Accessibility::SetWinEventHook;
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| unsafe {
        let _ = SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MINIMIZEEND, None, Some(fg_hook), 0, 0, WINEVENT_OUTOFCONTEXT);
        let _ = SetWinEventHook(EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE, None, Some(loc_hook), 0, 0, WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);
        let _ = SetWinEventHook(EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE, None, Some(loc_hook), 0, 0, WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);
    });
}

/// Cửa sổ đang chọn vừa đổi (đọc rồi xoá cờ).
pub fn foreground_changed() -> bool {
    FG_CHANGED.swap(false, std::sync::atomic::Ordering::Relaxed)
}

impl Drop for Layered {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.old);
            let _ = DeleteObject(self.bmp);
            let _ = DeleteDC(self.dc);
        }
    }
}

/// Biến cửa sổ winit thành thanh nổi trong suốt, không lấy focus, luôn ở trên.
const BAR_EX: WINDOW_EX_STYLE = WINDOW_EX_STYLE(WS_EX_LAYERED.0 | WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0 | WS_EX_TOPMOST.0);

/// Chặn mọi lệnh đổi kiểu cửa sổ taskbar (thư viện cửa sổ có lúc tự đặt lại kiểu thường,
/// làm taskbar hiện thành cửa sổ có thanh tiêu đề trắng): luôn giữ kiểu popup trong suốt.
unsafe extern "system" fn bar_subclass(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM, _id: usize, _data: usize) -> LRESULT {
    // Taskbar không có khung: không tính vùng khung, không vẽ khung / thanh tiêu đề (kể cả khi
    // Windows tạm vẽ cửa sổ kiểu thường lúc cập nhật ảnh trong suốt bị lỗi hoặc lúc kích hoạt).
    match msg {
        WM_NCCALCSIZE => return LRESULT(0),
        WM_NCPAINT => return LRESULT(0),
        WM_NCACTIVATE => return LRESULT(1),
        _ => {}
    }
    if msg == WM_STYLECHANGING && lp.0 != 0 {
        let ss = &mut *(lp.0 as *mut STYLESTRUCT);
        if wp.0 as i32 == GWL_STYLE.0 {
            ss.styleNew = WS_POPUP.0 | (ss.styleNew & WS_VISIBLE.0);
        } else if wp.0 as i32 == GWL_EXSTYLE.0 {
            ss.styleNew = (ss.styleNew | BAR_EX.0) & !(WS_EX_APPWINDOW.0 | WS_EX_WINDOWEDGE.0 | WS_EX_CLIENTEDGE.0);
        }
    }
    DefSubclassProc(h, msg, wp, lp)
}

pub fn make_bar_window(hwnd: HWND, x: i32, y: i32, w: i32, h: i32) {
    unsafe {
        let _ = SetWindowSubclass(hwnd, Some(bar_subclass), 0x5EA5, 0);
        SetWindowLongPtrW(hwnd, GWL_STYLE, (WS_POPUP | WS_VISIBLE).0 as isize);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, BAR_EX.0 as isize);
        let _ = SetWindowPos(hwnd, HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE | SWP_FRAMECHANGED);
    }
}

/// Cửa sổ taskbar còn đúng kiểu (không viền, trong suốt) không.
pub fn bar_style_ok(hwnd: HWND) -> bool {
    unsafe {
        let st = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        st & (WS_CAPTION.0 | WS_THICKFRAME.0 | WS_BORDER.0) == 0 && ex & WS_EX_LAYERED.0 != 0
    }
}

pub fn show(hwnd: HWND, on: bool) {
    unsafe {
        let _ = ShowWindow(hwnd, if on { SW_SHOWNOACTIVATE } else { SW_HIDE });
        if on {
            let _ = SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
    }
}

pub fn battery_percent() -> Option<u8> {
    use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    let mut s = SYSTEM_POWER_STATUS::default();
    unsafe {
        if GetSystemPowerStatus(&mut s).is_ok() && s.BatteryFlag != 128 && s.BatteryLifePercent <= 100 {
            return Some(s.BatteryLifePercent);
        }
    }
    None
}

// ------------------------------------------------------------------ icon khay hệ thống

/// Một icon ở khay hệ thống (đọc từ danh sách Windows 11 lưu trong
/// HKCU\Control Panel\NotifyIconSettings, có sẵn ảnh chụp icon thật).
#[allow(dead_code)]
pub struct TrayItem {
    pub exe: String,
    pub tip: String,
    pub png: Vec<u8>,
}

fn reg_get(sub: &str, value: &str) -> Option<Vec<u8>> {
    use windows::Win32::System::Registry::*;
    let s: Vec<u16> = sub.encode_utf16().chain(std::iter::once(0)).collect();
    let v: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let mut size = 0u32;
        if RegGetValueW(HKEY_CURRENT_USER, PCWSTR(s.as_ptr()), PCWSTR(v.as_ptr()), RRF_RT_ANY, None, None, Some(&mut size)).is_err() || size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        if RegGetValueW(HKEY_CURRENT_USER, PCWSTR(s.as_ptr()), PCWSTR(v.as_ptr()), RRF_RT_ANY, None, Some(buf.as_mut_ptr() as *mut _), Some(&mut size)).is_err() {
            return None;
        }
        buf.truncate(size as usize);
        Some(buf)
    }
}

/// Giá trị DWORD trong HKCU.
pub fn reg_dword(sub: &str, value: &str) -> Option<u32> {
    reg_get(sub, value).filter(|b| b.len() >= 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Icon thứ `idx` trong file (exe/dll/ico) ở kích thước `size`, hoặc icon của file qua shell.
pub fn icon_at(path: &str, idx: i32, size: i32) -> Option<Vec<u8>> {
    let low = path.to_lowercase();
    if low.ends_with(".exe") || low.ends_with(".dll") || low.ends_with(".ico") {
        let mut name = [0u16; 260];
        for (i, c) in path.encode_utf16().take(259).enumerate() {
            name[i] = c;
        }
        let mut icons = [HICON::default()];
        // Chỉ số âm = mã tài nguyên; PrivateExtractIcons chỉ nhận vị trí nên thử thẳng chỉ số dương.
        let n = unsafe { PrivateExtractIconsW(&name, idx.max(0), size, size, Some(&mut icons), None, 0) };
        if n != 0 && n != u32::MAX && !icons[0].is_invalid() {
            let out = icon_pixels(icons[0], size);
            unsafe {
                let _ = DestroyIcon(icons[0]);
            }
            if out.is_some() {
                return out;
            }
        }
    }
    file_icon(path, size)
}

/// Tên hiển thị của chương trình (FileDescription, ví dụ "Google Chrome"), như taskbar gốc.
pub fn exe_description(path: &str) -> Option<String> {
    use windows::Win32::Storage::FileSystem::*;
    let w: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let size = GetFileVersionInfoSizeW(PCWSTR(w.as_ptr()), None);
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        GetFileVersionInfoW(PCWSTR(w.as_ptr()), 0, size, buf.as_mut_ptr() as *mut _).ok()?;
        let mut p: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut len = 0u32;
        if !VerQueryValueW(buf.as_ptr() as *const _, w!(r"\VarFileInfo\Translation"), &mut p, &mut len).as_bool() || len < 4 {
            return None;
        }
        let tr = std::slice::from_raw_parts(p as *const u16, 2);
        let q: Vec<u16> = format!(r"\StringFileInfo\{:04x}{:04x}\FileDescription", tr[0], tr[1]).encode_utf16().chain(std::iter::once(0)).collect();
        if !VerQueryValueW(buf.as_ptr() as *const _, PCWSTR(q.as_ptr()), &mut p, &mut len).as_bool() || len == 0 {
            return None;
        }
        let s = String::from_utf16_lossy(std::slice::from_raw_parts(p as *const u16, len as usize));
        let s = s.trim_matches(char::from(0)).trim().to_string();
        (!s.is_empty()).then_some(s)
    }
}

fn utf16_bytes(b: &[u8]) -> String {
    let w: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|&c| c != 0).collect();
    String::from_utf16_lossy(&w)
}

/// Tên file (chữ thường) của các tiến trình đang chạy.
#[allow(dead_code)]
fn running_exes() -> std::collections::HashSet<String> {
    use windows::Win32::System::Diagnostics::ToolHelp::*;
    let mut set = std::collections::HashSet::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return set };
        let mut e = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        if Process32FirstW(snap, &mut e).is_ok() {
            loop {
                let n = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
                set.insert(String::from_utf16_lossy(&e.szExeFile[..n]).to_lowercase());
                if Process32NextW(snap, &mut e).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
    }
    set
}

/// Tên file của các app mà người dùng đã ghim icon ra ngoài khay (Windows lưu trong registry).
pub fn promoted_exes() -> std::collections::HashSet<String> {
    use windows::Win32::System::Registry::*;
    let mut set = std::collections::HashSet::new();
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, w!(r"Control Panel\NotifyIconSettings"), 0, KEY_READ, &mut key).is_err() {
            return set;
        }
        let mut i = 0u32;
        loop {
            let mut name = [0u16; 256];
            let mut len = name.len() as u32;
            if RegEnumKeyExW(key, i, windows::core::PWSTR(name.as_mut_ptr()), &mut len, None, windows::core::PWSTR::null(), None, None).is_err() {
                break;
            }
            i += 1;
            let sub = format!(r"Control Panel\NotifyIconSettings\{}", String::from_utf16_lossy(&name[..len as usize]));
            if reg_get(&sub, "IsPromoted").map(|b| b.first().copied().unwrap_or(0) == 1).unwrap_or(false) {
                let exe = reg_get(&sub, "ExecutablePath").map(|b| utf16_bytes(&b)).unwrap_or_default();
                if let Some(f) = exe.rsplit(['/', std::path::MAIN_SEPARATOR]).next() {
                    set.insert(f.to_lowercase());
                }
            }
        }
        let _ = RegCloseKey(key);
    }
    set
}

/// Các icon được ghim ra ngoài khay (không nằm trong mục ẩn) của app đang chạy.
#[allow(dead_code)]
pub fn pinned_tray_items() -> Vec<TrayItem> {
    use windows::Win32::System::Registry::*;
    let running = running_exes();
    let me = std::env::current_exe().ok().and_then(|p| p.file_name().map(|f| f.to_string_lossy().to_lowercase())).unwrap_or_default();
    let mut out: Vec<TrayItem> = Vec::new();
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, w!(r"Control Panel\NotifyIconSettings"), 0, KEY_READ, &mut key).is_err() {
            return out;
        }
        let mut i = 0u32;
        loop {
            let mut name = [0u16; 256];
            let mut len = name.len() as u32;
            let r = RegEnumKeyExW(key, i, windows::core::PWSTR(name.as_mut_ptr()), &mut len, None, windows::core::PWSTR::null(), None, None);
            if r.is_err() {
                break;
            }
            i += 1;
            let sub = format!(r"Control Panel\NotifyIconSettings\{}", String::from_utf16_lossy(&name[..len as usize]));
            let promoted = reg_get(&sub, "IsPromoted").map(|b| b.first().copied().unwrap_or(0) == 1).unwrap_or(false);
            if !promoted {
                continue;
            }
            let exe = reg_get(&sub, "ExecutablePath").map(|b| utf16_bytes(&b)).unwrap_or_default();
            let file = exe.rsplit(['/', std::path::MAIN_SEPARATOR]).next().unwrap_or("").to_lowercase();
            if file.is_empty() || file == me || !running.contains(&file) || out.iter().any(|t| t.exe == file) {
                continue;
            }
            let Some(png) = reg_get(&sub, "IconSnapshot") else { continue };
            let tip = reg_get(&sub, "InitialTooltip").map(|b| utf16_bytes(&b)).filter(|t| !t.is_empty()).unwrap_or_else(|| file.trim_end_matches(".exe").to_string());
            out.push(TrayItem { exe: file, tip, png });
        }
        let _ = RegCloseKey(key);
    }
    out
}

/// Mở khay hệ thống thật của Windows bằng bàn phím: Win+B đưa con trỏ vào khay (nút ^),
/// sau đó đi sang phải `right` bước rồi Enter (0 bước = mở bảng icon ẩn).
#[allow(dead_code)]
pub fn open_tray(right: usize) {
    std::thread::spawn(move || {
        press(&[KEY_WIN, VIRTUAL_KEY(0x42)]);
        std::thread::sleep(std::time::Duration::from_millis(350));
        for _ in 0..right {
            press(&[VK_RIGHT]);
            std::thread::sleep(std::time::Duration::from_millis(40));
        }
        press(&[VK_RETURN]);
    });
}


// ------------------------------------------------------------------ cửa sổ bảng icon ẩn

/// Sự kiện chuột của bảng icon ẩn (toạ độ pixel trong cửa sổ).
#[derive(Clone, Copy, Debug)]
pub enum PopupEvent {
    Move(i32, i32),
    Leave,
    Up { right: bool },
    /// Bấm chuột ra ngoài cửa sổ (menu phải đóng)
    Outside,
}

static WATCHED: std::sync::Mutex<Vec<isize>> = std::sync::Mutex::new(Vec::new());
static HOOK: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

unsafe extern "system" fn ll_mouse(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code >= 0 && matches!(wp.0 as u32, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_NCLBUTTONDOWN | WM_NCRBUTTONDOWN) {
        let pt = (*(lp.0 as *const MSLLHOOKSTRUCT)).pt;
        let list = WATCHED.lock().map(|v| v.clone()).unwrap_or_default();
        for m in list {
            let mut r = RECT::default();
            let _ = GetWindowRect(HWND(m as *mut _), &mut r);
            if pt.x < r.left || pt.x >= r.right || pt.y < r.top || pt.y >= r.bottom {
                push_popup(HWND(m as *mut _), PopupEvent::Outside);
            }
        }
    }
    CallNextHookEx(HHOOK::default(), code, wp, lp)
}

/// Bật/tắt theo dõi cú bấm chuột ra ngoài cửa sổ nổi `h` (menu, lịch, bảng ^ đóng lại như Windows).
pub fn watch_outside(h: HWND, on: bool) {
    use std::sync::atomic::Ordering::Relaxed;
    let id = h.0 as isize;
    let empty = {
        let Ok(mut v) = WATCHED.lock() else { return };
        v.retain(|x| *x != id);
        if on {
            v.push(id);
        }
        v.is_empty()
    };
    unsafe {
        if !empty && HOOK.load(Relaxed) == 0 {
            if let Ok(hk) = SetWindowsHookExW(WH_MOUSE_LL, Some(ll_mouse), None, 0) {
                HOOK.store(hk.0 as isize, Relaxed);
            }
        } else if empty {
            let hk = HOOK.swap(0, Relaxed);
            if hk != 0 {
                let _ = UnhookWindowsHookEx(HHOOK(hk as *mut _));
            }
        }
    }
}

static POPUP_EVENTS: std::sync::Mutex<Vec<(isize, PopupEvent)>> = std::sync::Mutex::new(Vec::new());

fn push_popup(h: HWND, e: PopupEvent) {
    if let Ok(mut v) = POPUP_EVENTS.lock() {
        v.push((h.0 as isize, e));
    }
}

/// Lấy sự kiện chuột của một cửa sổ nổi (giữ lại sự kiện của cửa sổ khác).
pub fn take_popup_events(h: HWND) -> Vec<PopupEvent> {
    let id = h.0 as isize;
    let Ok(mut v) = POPUP_EVENTS.lock() else { return Vec::new() };
    let (mine, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut *v).into_iter().partition(|(w, _)| *w == id);
    *v = rest;
    mine.into_iter().map(|(_, e)| e).collect()
}

unsafe extern "system" fn popup_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let pt = || ((lp.0 & 0xFFFF) as i16 as i32, ((lp.0 >> 16) & 0xFFFF) as i16 as i32);
    match msg {
        WM_MOUSEMOVE => {
            {
                let mut tme = TRACKMOUSEEVENT { cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE, hwndTrack: h, dwHoverTime: 0 };
                let _ = TrackMouseEvent(&mut tme);
            }
            let (x, y) = pt();
            push_popup(h, PopupEvent::Move(x, y));
            LRESULT(0)
        }
        0x02A3 /* WM_MOUSELEAVE */ => {
            push_popup(h, PopupEvent::Leave);
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            push_popup(h, PopupEvent::Up { right: false });
            LRESULT(0)
        }
        WM_RBUTTONUP => {
            push_popup(h, PopupEvent::Up { right: true });
            LRESULT(0)
        }
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        _ => DefWindowProcW(h, msg, wp, lp),
    }
}

/// Tạo cửa sổ nổi trong suốt (layered) dùng cho bảng icon ẩn; ban đầu ẩn.
pub fn create_popup() -> Option<HWND> {
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    unsafe {
        let inst = GetModuleHandleW(PCWSTR::null()).ok()?;
        let wc = WNDCLASSW { lpfnWndProc: Some(popup_proc), hInstance: inst.into(), lpszClassName: w!("SeasonDesktopPopup"), hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(), ..Default::default() };
        RegisterClassW(&wc);
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
            w!("SeasonDesktopPopup"),
            w!("Season Desktop"),
            WS_POPUP,
            0,
            0,
            10,
            10,
            HWND::default(),
            HMENU::default(),
            inst,
            None,
        )
        .ok()
    }
}

/// Ảnh đã vẽ sẵn của các cửa sổ xem trước (BGRA, từ trên xuống), vẽ ra khi Windows yêu cầu.
static PREVIEW_BITS: std::sync::Mutex<Vec<(isize, i32, i32, Vec<u8>)>> = std::sync::Mutex::new(Vec::new());

unsafe extern "system" fn preview_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_PAINT {
        let mut ps = PAINTSTRUCT::default();
        let dc = BeginPaint(h, &mut ps);
        if let Ok(v) = PREVIEW_BITS.lock() {
            if let Some((_, w, hh, bits)) = v.iter().find(|e| e.0 == h.0 as isize) {
                let bi = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER { biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32, biWidth: *w, biHeight: -*hh, biPlanes: 1, biBitCount: 32, biCompression: BI_RGB.0, ..Default::default() },
                    ..Default::default()
                };
                SetDIBitsToDevice(dc, 0, 0, *w as u32, *hh as u32, 0, 0, 0, *hh as u32, bits.as_ptr() as *const _, &bi, DIB_RGB_COLORS);
            }
        }
        let _ = EndPaint(h, &ps);
        return LRESULT(0);
    }
    if msg == WM_ERASEBKGND {
        return LRESULT(1);
    }
    popup_proc(h, msg, wp, lp)
}

/// Cửa sổ nổi thường (không trong suốt) để hiện ảnh thu nhỏ của cửa sổ khác (DWM không vẽ lên cửa sổ layered).
pub fn create_preview() -> Option<HWND> {
    use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    unsafe {
        let inst = GetModuleHandleW(PCWSTR::null()).ok()?;
        let wc = WNDCLASSW { lpfnWndProc: Some(preview_proc), hInstance: inst.into(), lpszClassName: w!("SeasonDesktopPreview"), hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(), ..Default::default() };
        RegisterClassW(&wc);
        let h = CreateWindowExW(WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST, w!("SeasonDesktopPreview"), w!("Season Desktop"), WS_POPUP, 0, 0, 10, 10, HWND::default(), HMENU::default(), inst, None).ok()?;
        let round: u32 = 2; // DWMWCP_ROUND: bo góc như Windows 11
        let _ = DwmSetWindowAttribute(h, DWMWA_WINDOW_CORNER_PREFERENCE, &round as *const u32 as *const _, 4);
        Some(h)
    }
}

/// Độ trong suốt của cả cửa sổ (0 = vô hình, 255 = rõ hẳn).
pub fn set_alpha(h: HWND, a: u8) {
    unsafe {
        let _ = SetLayeredWindowAttributes(h, COLORREF(0), a, LWA_ALPHA);
    }
}

/// Dời cửa sổ tới (x, y), giữ kích thước.
pub fn move_window(h: HWND, x: i32, y: i32) {
    unsafe {
        let _ = SetWindowPos(h, HWND::default(), x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
    }
}

/// Đưa ảnh (RGBA đã nhân alpha, nền đặc) lên cửa sổ xem trước.
pub fn preview_paint(h: HWND, w: i32, hh: i32, rgba: &[u8]) {
    let mut bgra = rgba.to_vec();
    for p in bgra.chunks_exact_mut(4) {
        p.swap(0, 2);
    }
    if let Ok(mut v) = PREVIEW_BITS.lock() {
        v.retain(|e| e.0 != h.0 as isize);
        v.push((h.0 as isize, w, hh, bgra));
    }
    unsafe {
        let _ = InvalidateRect(h, None, false);
        let _ = UpdateWindow(h);
    }
}

/// Ảnh thu nhỏ trực tiếp của cửa sổ `src` vẽ lên `dest` trong vùng `area` (giữ đúng tỉ lệ).
pub fn thumb_register(dest: HWND, src: HWND, area: RECT) -> Option<isize> {
    use windows::Win32::Graphics::Dwm::*;
    unsafe {
        let t = DwmRegisterThumbnail(dest, src).ok()?;
        let size = DwmQueryThumbnailSourceSize(t).unwrap_or_default();
        let (aw, ah) = ((area.right - area.left) as f32, (area.bottom - area.top) as f32);
        let mut r = area;
        if size.cx > 0 && size.cy > 0 {
            let s = (aw / size.cx as f32).min(ah / size.cy as f32).min(1.0);
            let (w, h) = ((size.cx as f32 * s).round() as i32, (size.cy as f32 * s).round() as i32);
            r.left = area.left + ((aw as i32 - w) / 2);
            r.top = area.top + ((ah as i32 - h) / 2);
            r.right = r.left + w;
            r.bottom = r.top + h;
        }
        let props = DWM_THUMBNAIL_PROPERTIES {
            dwFlags: DWM_TNP_RECTDESTINATION | DWM_TNP_VISIBLE | DWM_TNP_OPACITY | DWM_TNP_SOURCECLIENTAREAONLY,
            rcDestination: r,
            opacity: 255,
            fVisible: true.into(),
            fSourceClientAreaOnly: false.into(),
            ..Default::default()
        };
        let _ = DwmUpdateThumbnailProperties(t, &props);
        Some(t)
    }
}

pub fn thumb_unregister(t: isize) {
    unsafe {
        let _ = windows::Win32::Graphics::Dwm::DwmUnregisterThumbnail(t);
    }
}

/// Tiêu đề của cửa sổ.
pub fn window_title(h: isize) -> String {
    let mut buf = [0u16; 512];
    let n = unsafe { GetWindowTextW(HWND(h as *mut _), &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

pub fn is_minimized(h: isize) -> bool {
    unsafe { IsIconic(HWND(h as *mut _)).as_bool() }
}

pub fn move_popup(h: HWND, x: i32, y: i32, w: i32, hh: i32) {
    unsafe {
        let _ = SetWindowPos(h, HWND_TOPMOST, x, y, w, hh, SWP_NOACTIVATE);
    }
}

pub fn destroy_popup(h: HWND) {
    unsafe {
        let _ = DestroyWindow(h);
    }
}


/// Tắt các bản Season Desktop khác đang chạy (dùng khi gỡ cài đặt).
/// `keep_watchdogs` = giữ lại tiến trình canh (chúng chạy bằng tài khoản người dùng và sẽ
/// tự trả taskbar gốc ngay khi app chính tắt), rồi chờ chúng làm xong (tối đa 4 giây).
pub fn close_instances(keep_watchdogs: bool) {
    let parents: Vec<(u32, u32)> = season_processes();
    let me = std::process::id();
    let pids: Vec<u32> = parents.iter().map(|p| p.0).collect();
    for &(pid, parent) in &parents {
        if pid == me {
            continue;
        }
        let is_watchdog = pids.contains(&parent);
        if keep_watchdogs && is_watchdog {
            continue;
        }
        terminate(pid);
    }
    if keep_watchdogs {
        let t0 = std::time::Instant::now();
        while t0.elapsed() < std::time::Duration::from_secs(4) && season_processes().iter().any(|p| p.0 != me) {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        // còn sót thì tắt hẳn
        for (pid, _) in season_processes() {
            if pid != me {
                terminate(pid);
            }
        }
    }
}

/// (pid, pid cha) của các tiến trình season-desktop.exe.
fn season_processes() -> Vec<(u32, u32)> {
    use windows::Win32::System::Diagnostics::ToolHelp::*;
    let mut out = Vec::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return out };
        let mut e = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        if Process32FirstW(snap, &mut e).is_ok() {
            loop {
                let n = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
                if String::from_utf16_lossy(&e.szExeFile[..n]).eq_ignore_ascii_case("season-desktop.exe") {
                    out.push((e.th32ProcessID, e.th32ParentProcessID));
                }
                if Process32NextW(snap, &mut e).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
    }
    out
}

fn terminate(pid: u32) {
    use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
    unsafe {
        if let Ok(h) = OpenProcess(PROCESS_TERMINATE, false, pid) {
            let _ = TerminateProcess(h, 0);
            let _ = CloseHandle(h);
        }
    }
}

#[allow(dead_code)]
pub fn close_other_instances() {
    use windows::Win32::System::Diagnostics::ToolHelp::*;
    use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
    let me = std::process::id();
    let my_name = std::env::current_exe().ok().and_then(|p| p.file_name().map(|f| f.to_string_lossy().to_lowercase())).unwrap_or_default();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return };
        let mut e = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        if Process32FirstW(snap, &mut e).is_ok() {
            loop {
                let n = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
                let name = String::from_utf16_lossy(&e.szExeFile[..n]).to_lowercase();
                if e.th32ProcessID != me && (name == my_name || name == "season-desktop.exe") {
                    if let Ok(h) = OpenProcess(PROCESS_TERMINATE, false, e.th32ProcessID) {
                        let _ = TerminateProcess(h, 0);
                        let _ = CloseHandle(h);
                    }
                }
                if Process32NextW(snap, &mut e).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
    }
}


// ------------------------------------------------------------------ nhận yêu cầu từ bộ cài

/// 0 = không có, 1 = thoát (nâng cấp), 2 = gỡ cài đặt (thoát + trả mọi thứ về gốc),
/// 3 = mở cửa sổ cài đặt (người dùng mở app lần nữa trong khi app đang chạy).
pub static CONTROL_REQUEST: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn control_msgs() -> (u32, u32) {
    unsafe { (RegisterWindowMessageW(w!("SeasonDesktop.Quit")), RegisterWindowMessageW(w!("SeasonDesktop.Uninstall"))) }
}

fn open_msg() -> u32 {
    unsafe { RegisterWindowMessageW(w!("SeasonDesktop.Open")) }
}

unsafe extern "system" fn control_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let (quit, uninstall) = control_msgs();
    if msg != 0 && msg == open_msg() {
        let _ = CONTROL_REQUEST.compare_exchange(0, 3, std::sync::atomic::Ordering::SeqCst, std::sync::atomic::Ordering::SeqCst);
        return LRESULT(1);
    }
    if msg != 0 && msg == uninstall {
        CONTROL_REQUEST.store(2, std::sync::atomic::Ordering::SeqCst);
        return LRESULT(1);
    }
    if msg != 0 && msg == quit {
        let _ = CONTROL_REQUEST.compare_exchange(0, 1, std::sync::atomic::Ordering::SeqCst, std::sync::atomic::Ordering::SeqCst);
        return LRESULT(1);
    }
    DefWindowProcW(h, msg, wp, lp)
}

/// Cửa sổ ẩn để bộ cài nhắn app tự thoát (và tự dọn khi gỡ cài đặt).
pub fn create_control_window() -> Option<HWND> {
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    unsafe {
        let inst = GetModuleHandleW(PCWSTR::null()).ok()?;
        let wc = WNDCLASSW { lpfnWndProc: Some(control_proc), hInstance: inst.into(), lpszClassName: w!("SeasonDesktopControl"), ..Default::default() };
        RegisterClassW(&wc);
        // Cho phép tin nhắn đăng ký đi qua dù tiến trình gửi khác mức quyền.
        let h = CreateWindowExW(WS_EX_TOOLWINDOW, w!("SeasonDesktopControl"), w!("Season Desktop control"), WS_POPUP, 0, 0, 0, 0, HWND::default(), HMENU::default(), inst, None).ok()?;
        let (q, u) = control_msgs();
        let _ = ChangeWindowMessageFilterEx(h, q, MSGFLT_ALLOW, None);
        let _ = ChangeWindowMessageFilterEx(h, u, MSGFLT_ALLOW, None);
        let _ = ChangeWindowMessageFilterEx(h, open_msg(), MSGFLT_ALLOW, None);
        Some(h)
    }
}

/// Nhắn bản app đang chạy mở cửa sổ cài đặt (và cho nó quyền đưa cửa sổ lên trước).
pub fn send_open() -> bool {
    unsafe {
        let _ = AllowSetForegroundWindow(ASFW_ANY);
        match FindWindowExW(HWND::default(), HWND::default(), w!("SeasonDesktopControl"), PCWSTR::null()) {
            Ok(h) => PostMessageW(h, open_msg(), WPARAM(0), LPARAM(0)).is_ok(),
            Err(_) => false,
        }
    }
}

/// Gửi yêu cầu cho mọi bản app đang chạy; trả về số bản đã nhận.
pub fn send_control(uninstall: bool) -> usize {
    let (q, u) = control_msgs();
    let m = if uninstall { u } else { q };
    let mut n = 0;
    unsafe {
        let mut prev = HWND::default();
        while let Ok(h) = FindWindowExW(HWND::default(), prev, w!("SeasonDesktopControl"), PCWSTR::null()) {
            if PostMessageW(h, m, WPARAM(0), LPARAM(0)).is_ok() {
                n += 1;
            }
            prev = h;
        }
    }
    n
}

/// Chờ các bản app khác thoát (tối đa `secs` giây). Trả về true nếu đã thoát hết.
pub fn wait_instances_exit(secs: u64) -> bool {
    let me = std::process::id();
    let t0 = std::time::Instant::now();
    while t0.elapsed() < std::time::Duration::from_secs(secs) {
        if !season_processes().iter().any(|p| p.0 != me) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    false
}


// ------------------------------------------------------------------ app đã ghim trên taskbar

/// Một app người dùng đã ghim trên taskbar Windows.
pub struct Pin {
    /// File .lnk trong thư mục ghim của Windows
    pub lnk: String,
    /// Đường dẫn chương trình (chữ thường) để khớp với cửa sổ đang mở; rỗng với app Store
    pub target: String,
    pub name: String,
    pub icon: Option<Vec<u8>>,
    /// AppUserModelID (Windows dùng để gộp cửa sổ vào đúng ô ghim)
    pub aumid: String,
}

/// Thời điểm tiến trình của cửa sổ được mở (đơn vị 100 ns, FILETIME). Dùng để xếp các app đang
/// mở theo thứ tự mở như taskbar gốc khi thanh vừa khởi động. 0 = không đọc được.
pub fn process_started(hwnd: isize) -> u64 {
    use windows::Win32::Foundation::{CloseHandle, FILETIME};
    use windows::Win32::System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(HWND(hwnd as *mut _), Some(&mut pid));
        if pid == 0 {
            return 0;
        }
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return 0 };
        let (mut c, mut e, mut k, mut u) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
        let ok = GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u).is_ok();
        let _ = CloseHandle(h);
        if ok { ((c.dwHighDateTime as u64) << 32) | c.dwLowDateTime as u64 } else { 0 }
    }
}

/// ID app (AUMID) của cửa sổ thuộc app đóng gói (Store/MSIX), lấy từ gói của tiến trình –
/// đúng ID mà taskbar gốc dùng, kể cả khi app không tự gắn ID lên cửa sổ.
pub fn package_aumid(hwnd: isize) -> Option<String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Storage::Packaging::Appx::GetApplicationUserModelId;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(HWND(hwnd as *mut _), Some(&mut pid));
        if pid == 0 {
            return None;
        }
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 256];
        let mut len = buf.len() as u32;
        let r = GetApplicationUserModelId(h, &mut len, windows::core::PWSTR(buf.as_mut_ptr()));
        let _ = CloseHandle(h);
        if r.is_err() {
            return None;
        }
        let n = buf.iter().position(|&c| c == 0).unwrap_or(0);
        (n > 0).then(|| String::from_utf16_lossy(&buf[..n]))
    }
}

/// ID app (AUMID) ghi trong lối tắt Start Menu trỏ tới chương trình `exe` (đường dẫn chữ thường):
/// cách Windows gán ID cho app không tự gắn (ví dụ VS Code). Đọc một lần ở luồng nền.
pub fn start_menu_aumid(exe: &str) -> Option<String> {
    start_menu().and_then(|m| m.get(exe).map(|(a, _)| a.clone())).filter(|a| !a.is_empty())
}

/// Bảng chương trình → (ID app, lối tắt) đọc từ Start Menu ở luồng nền (lần đầu gọi thì bắt đầu đọc).
fn start_menu() -> Option<std::collections::HashMap<String, (String, String)>> {
    use std::collections::HashMap;
    static MAP: std::sync::Mutex<Option<HashMap<String, (String, String)>>> = std::sync::Mutex::new(None);
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let _ = std::thread::Builder::new().name("start-menu".into()).spawn(|| {
            let mut map = HashMap::new();
            let mut dirs: Vec<std::path::PathBuf> = Vec::new();
            for (var, sub) in [("ProgramData", r"Microsoft\Windows\Start Menu\Programs"), ("APPDATA", r"Microsoft\Windows\Start Menu\Programs")] {
                if let Some(base) = std::env::var_os(var) {
                    dirs.push(std::path::PathBuf::from(base).join(sub));
                }
            }
            while let Some(d) = dirs.pop() {
                let Ok(rd) = std::fs::read_dir(&d) else { continue };
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        dirs.push(p);
                    } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("lnk")) {
                        let l = p.to_string_lossy().to_string();
                        if let Some(t) = lnk_target(&l).filter(|t| !t.is_empty()) {
                            let a = super::jumplist::lnk_aumid(&l).unwrap_or_default();
                            map.entry(t.to_lowercase()).or_insert((a, l));
                        }
                    }
                }
            }
            if let Ok(mut m) = MAP.lock() {
                *m = Some(map);
            }
        });
    });
    MAP.lock().ok()?.clone()
}

/// Đường dẫn chương trình của cửa sổ (chữ thường).
pub fn exe_path(hwnd: isize) -> String {
    exe_of(HWND(hwnd as *mut _)).map(|p| p.to_lowercase()).unwrap_or_default()
}

fn lnk_target(lnk: &str) -> Option<String> {
    use windows::core::Interface;
    use windows::Win32::System::Com::*;
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let sl: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let pf: IPersistFile = sl.cast().ok()?;
        let w: Vec<u16> = lnk.encode_utf16().chain(std::iter::once(0)).collect();
        pf.Load(PCWSTR(w.as_ptr()), STGM_READ).ok()?;
        let mut buf = [0u16; 520];
        sl.GetPath(&mut buf, std::ptr::null_mut(), 0).ok()?;
        let n = buf.iter().position(|&c| c == 0).unwrap_or(0);
        if n == 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..n]))
    }
}

/// Icon (RGBA đã nhân alpha) của một file bất kỳ (exe hoặc .lnk) qua shell.
fn file_icon(path: &str, size: i32) -> Option<Vec<u8>> {
    let w: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    if path.to_lowercase().ends_with(".exe") {
        let mut name = [0u16; 260];
        for (i, c) in path.encode_utf16().take(259).enumerate() {
            name[i] = c;
        }
        let mut icons = [HICON::default()];
        let n = unsafe { PrivateExtractIconsW(&name, 0, size, size, Some(&mut icons), None, 0) };
        if n != 0 && n != u32::MAX && !icons[0].is_invalid() {
            let out = icon_pixels(icons[0], size);
            unsafe {
                let _ = DestroyIcon(icons[0]);
            }
            if out.is_some() {
                return out;
            }
        }
    }
    unsafe {
        // Icon gốc trong bộ icon hệ thống: không kèm mũi tên lối tắt (giống taskbar gốc).
        let mut sfi = SHFILEINFOW::default();
        let r = SHGetFileInfoW(PCWSTR(w.as_ptr()), windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES(0), Some(&mut sfi), std::mem::size_of::<SHFILEINFOW>() as u32, SHGFI_SYSICONINDEX);
        if r != 0 {
            use windows::Win32::UI::Controls::{IImageList, ILD_TRANSPARENT};
            let which = if size > 32 { SHIL_EXTRALARGE } else if size > 16 { SHIL_LARGE } else { SHIL_SMALL };
            if let Ok(il) = SHGetImageList::<IImageList>(which as i32) {
                if let Ok(h) = il.GetIcon(sfi.iIcon, ILD_TRANSPARENT.0) {
                    let out = icon_pixels(h, size);
                    let _ = DestroyIcon(h);
                    if out.is_some() {
                        return out;
                    }
                }
            }
        }
        let mut sfi = SHFILEINFOW::default();
        let r = SHGetFileInfoW(PCWSTR(w.as_ptr()), windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES(0), Some(&mut sfi), std::mem::size_of::<SHFILEINFOW>() as u32, SHGFI_ICON | SHGFI_LARGEICON);
        if r == 0 || sfi.hIcon.is_invalid() {
            return None;
        }
        let out = icon_pixels(sfi.hIcon, size);
        let _ = DestroyIcon(sfi.hIcon);
        out
    }
}

/// Một app ghim đọc từ lối tắt (.lnk).
fn pin_from_lnk(lnk: &str, size: i32) -> Pin {
    let path = std::path::Path::new(lnk);
    let name = path.file_stem().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
    let aumid = super::jumplist::lnk_aumid(lnk).unwrap_or_default();
    let mut target = lnk_target(lnk).unwrap_or_default();
    // Lối tắt File Explorer không trỏ tới file nào: cửa sổ của nó là explorer.exe
    if target.is_empty() && aumid == "Microsoft.Windows.Explorer" {
        target = format!(r"{}\explorer.exe", std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into()));
    }
    // Icon riêng ghi trong lối tắt (ví dụ ảnh hồ sơ Chrome), rồi tới icon chương trình, cuối cùng icon của lối tắt
    let icon = lnk_icon_location(lnk)
        .and_then(|(p, i)| icon_at(&p, i, size))
        .or_else(|| aumid_icon(&aumid, size))
        .or_else(|| if !target.is_empty() && std::path::Path::new(&target).exists() { file_icon(&target, size) } else { None })
        .or_else(|| file_icon(lnk, size));
    Pin { lnk: lnk.to_string(), target: target.to_lowercase(), name, icon, aumid }
}

/// Icon ghi riêng trong lối tắt: (file, chỉ số).
fn lnk_icon_location(lnk: &str) -> Option<(String, i32)> {
    use windows::core::Interface;
    use windows::Win32::System::Com::*;
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let sl: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let pf: IPersistFile = sl.cast().ok()?;
        pf.Load(PCWSTR(wide(lnk).as_ptr()), STGM_READ).ok()?;
        let mut buf = [0u16; 1024];
        let mut idx = 0i32;
        sl.GetIconLocation(&mut buf, &mut idx).ok()?;
        let n = buf.iter().position(|&c| c == 0).unwrap_or(0);
        if n == 0 {
            return None;
        }
        let p = expand_env(&String::from_utf16_lossy(&buf[..n]));
        std::path::Path::new(&p).exists().then_some((p, idx))
    }
}

fn expand_env(s: &str) -> String {
    if !s.contains('%') {
        return s.to_string();
    }
    let mut out = vec![0u16; 2048];
    let n = unsafe { windows::Win32::System::Environment::ExpandEnvironmentStringsW(PCWSTR(wide(s).as_ptr()), Some(&mut out)) };
    if n == 0 {
        return s.to_string();
    }
    let n = out.iter().position(|&c| c == 0).unwrap_or(0);
    String::from_utf16_lossy(&out[..n])
}

/// Ảnh icon (RGBA đã nhân alpha) của một mục shell (app Store, thư mục...) ở đúng kích thước.
fn item_icon(item: &IShellItem, size: i32) -> Option<Vec<u8>> {
    use windows::core::Interface;
    unsafe {
        let f: IShellItemImageFactory = item.cast().ok()?;
        let hb = f.GetImage(SIZE { cx: size, cy: size }, SIIGBF_ICONONLY).ok()?;
        let out = bitmap_rgba(hb, size);
        let _ = DeleteObject(hb);
        out
    }
}

fn bitmap_rgba(hb: HBITMAP, size: i32) -> Option<Vec<u8>> {
    unsafe {
        let dc = GetDC(HWND::default());
        let mut bi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER { biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32, biWidth: size, biHeight: -size, biPlanes: 1, biBitCount: 32, biCompression: BI_RGB.0, ..Default::default() },
            ..Default::default()
        };
        let mut px = vec![0u8; (size * size * 4) as usize];
        let n = GetDIBits(dc, hb, 0, size as u32, Some(px.as_mut_ptr() as *mut _), &mut bi, DIB_RGB_COLORS);
        ReleaseDC(HWND::default(), dc);
        if n == 0 {
            return None;
        }
        let opaque = px.chunks_exact(4).all(|p| p[3] == 0);
        for p in px.chunks_exact_mut(4) {
            p.swap(0, 2);
            if opaque {
                p[3] = 255;
            }
            // Bảo đảm đúng dạng đã nhân alpha
            let a = p[3];
            p[0] = p[0].min(a);
            p[1] = p[1].min(a);
            p[2] = p[2].min(a);
        }
        Some(px)
    }
}

/// Logo mà mục ghim app Store tự lưu (thư mục cài + "…44x44Logo.png"), như taskbar gốc dùng
/// khi không tra được app theo ID. Chọn bản đúng cỡ (targetsize-N, ưu tiên bản không nền).
fn logo_in_idlist(idl: &[u8], size: i32) -> Option<Vec<u8>> {
    // Các chuỗi UTF-16 (chữ ASCII) trong mục ghim, đọc ở cả hai vị trí chẵn/lẻ của byte
    let mut strs: Vec<String> = Vec::new();
    for start in 0..2 {
        let mut cur = String::new();
        for c in idl[start..].chunks_exact(2) {
            let ch = u16::from_le_bytes([c[0], c[1]]);
            if (0x20..0x7F).contains(&ch) {
                cur.push(ch as u8 as char);
            } else {
                if cur.len() >= 4 {
                    strs.push(std::mem::take(&mut cur));
                }
                cur.clear();
            }
        }
    }
    // Thư mục cài: phần bắt đầu bằng "X:\" (bỏ ký tự rác phía trước)
    let dirs: Vec<std::path::PathBuf> = strs
        .iter()
        .filter_map(|s| {
            let b = s.as_bytes();
            (1..b.len().saturating_sub(1)).find(|&i| b[i] == b':' && b[i + 1] == b'\\' && b[i - 1].is_ascii_alphabetic()).map(|i| std::path::PathBuf::from(&s[i - 1..]))
        })
        .filter(|p| p.is_dir())
        .collect();
    let rel_raw = strs.iter().find(|s| s.to_lowercase().contains("44x44logo") && s.to_lowercase().ends_with(".png"))?;
    let parts: Vec<&str> = rel_raw.split('\\').collect();
    for d in dirs {
        // Phần đầu của đường dẫn tương đối có thể dính ký tự rác: thử bỏ dần từng phần đầu
        let first = parts[0];
        let rest = parts[1..].join("\\");
        let has_logo = |f: &std::path::Path| {
            let (Some(p), Some(stem)) = (f.parent(), f.file_stem().map(|s| s.to_string_lossy().to_lowercase())) else { return false };
            std::fs::read_dir(p).map(|r| r.flatten().any(|e| e.file_name().to_string_lossy().to_lowercase().starts_with(&stem))).unwrap_or(false)
        };
        let Some(full) = (0..first.len())
            .filter(|&j| first.is_char_boundary(j))
            .map(|j| if rest.is_empty() { d.join(&first[j..]) } else { d.join(format!("{}\\{}", &first[j..], rest)) })
            .find(|f| has_logo(f))
        else {
            continue;
        };
        let (Some(dir), Some(stem)) = (full.parent(), full.file_stem().map(|s| s.to_string_lossy().to_string())) else { continue };
        if let Some(v) = best_logo(dir, &stem, size) {
            return Some(v);
        }
    }
    None
}

/// Chọn ảnh logo đẹp nhất cho cỡ `size` trong thư mục `dir` (các file bắt đầu bằng `stem`):
/// ưu tiên bản đúng cỡ (targetsize-N) và không nền (altform-unplated) như taskbar gốc dùng,
/// bỏ bản tương phản cao. Trả về RGBA đã nhân alpha, đúng size x size.
fn best_logo(dir: &std::path::Path, stem: &str, size: i32) -> Option<Vec<u8>> {
    let stem = stem.to_lowercase();
    let mut best: Option<(i32, std::path::PathBuf)> = None;
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let name = e.file_name().to_string_lossy().to_lowercase();
        if !name.starts_with(&stem) || !name.ends_with(".png") || name.contains("lightunplated") || name.contains("contrast") {
            continue;
        }
        let n = name.split("targetsize-").nth(1).and_then(|t| t.split(|c: char| !c.is_ascii_digit()).next()).and_then(|t| t.parse::<i32>().ok());
        let score = match n {
            Some(n) => (if n >= size { 1000 - (n - size) } else { 500 - (size - n) }) + if name.contains("unplated") { 5 } else { 0 },
            None => 100,
        };
        if best.as_ref().map(|b| score > b.0).unwrap_or(true) {
            best = Some((score, e.path()));
        }
    }
    let (_, p) = best?;
    let png = tiny_skia::Pixmap::decode_png(&std::fs::read(p).ok()?).ok()?;
    let mut out = tiny_skia::Pixmap::new(size as u32, size as u32)?;
    let s = size as f32 / png.width().max(png.height()).max(1) as f32;
    let (ox, oy) = ((size as f32 - png.width() as f32 * s) / 2.0, (size as f32 - png.height() as f32 * s) / 2.0);
    let q = if (s - 1.0).abs() < 0.01 { tiny_skia::FilterQuality::Nearest } else { tiny_skia::FilterQuality::Bicubic };
    out.draw_pixmap(0, 0, png.as_ref(), &tiny_skia::PixmapPaint { quality: q, ..Default::default() }, tiny_skia::Transform::from_translate(ox.round(), oy.round()).pre_scale(s, s), None);
    Some(out.data().to_vec())
}

/// Logo không nền đúng cỡ của app Store, lấy từ thư mục cài của gói (như taskbar gốc).
fn store_logo(item: &IShellItem, size: i32) -> Option<Vec<u8>> {
    use windows::core::{Interface, GUID};
    use windows::Win32::UI::Shell::PropertiesSystem::PROPERTYKEY;
    const INSTALL_PATH: PROPERTYKEY = PROPERTYKEY { fmtid: GUID::from_u128(0x9F4C2855_9F79_4B39_A8D0_E1D42DE1D5F3), pid: 15 };
    const SMALL_LOGO: PROPERTYKEY = PROPERTYKEY { fmtid: GUID::from_u128(0x86D40B4D_9069_443C_819A_2A54090DCCEC), pid: 2 };
    unsafe {
        let it2: IShellItem2 = item.cast().ok()?;
        let take = |p: windows::core::PWSTR| {
            let s = p.to_string().unwrap_or_default();
            windows::Win32::System::Com::CoTaskMemFree(Some(p.0 as *const _));
            s
        };
        let dir = take(it2.GetString(&INSTALL_PATH).ok()?);
        let rel = take(it2.GetString(&SMALL_LOGO).ok()?);
        if dir.is_empty() || rel.is_empty() {
            return None;
        }
        let full = std::path::Path::new(&dir).join(&rel);
        let stem = full.file_stem()?.to_string_lossy().to_string();
        best_logo(full.parent()?, &stem, size)
    }
}

/// Icon của app Store (hoặc app bất kỳ) theo AppUserModelID.
pub fn aumid_icon(aumid: &str, size: i32) -> Option<Vec<u8>> {
    if aumid.is_empty() {
        return None;
    }
    unsafe {
        let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(wide(&format!(r"shell:AppsFolder\{aumid}")).as_ptr()), None).ok()?;
        store_logo(&item, size).or_else(|| item_icon(&item, size))
    }
}

/// Icon nên dùng cho cửa sổ, giống taskbar gốc: icon riêng mà app khai báo cho cửa sổ (ví dụ
/// ảnh hồ sơ Chrome), app Store thì icon theo ID của app, còn lại icon chương trình.
pub fn window_icon_for(hwnd: isize, exe: &str, aumid: &str, size: i32) -> Option<Vec<u8>> {
    if let Some((p, i)) = super::jumplist::relaunch_icon(hwnd) {
        if let Some(v) = icon_at(&p, i, size) {
            return Some(v);
        }
    }
    if exe.ends_with("applicationframehost.exe") || exe.contains(r"\windowsapps\") {
        if let Some(v) = aumid_icon(aumid, size) {
            return Some(v);
        }
    }
    window_icon(hwnd, size)
        .or_else(|| aumid_icon(aumid, size))
        .or_else(|| unsafe { LoadIconW(None, IDI_APPLICATION).ok().and_then(|icon| icon_pixels(icon, size)) })
}

/// Các mục ghim lưu trong Taskband\Favorites (đúng thứ tự của taskbar gốc, gồm cả app Store).
fn favorites(size: i32) -> Option<Vec<Pin>> {
    let fav = reg_get(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Taskband", "Favorites")?;
    let mut out = Vec::new();
    let mut p = 1usize;
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED);
        while p + 4 <= fav.len() {
            let n = u32::from_le_bytes([fav[p], fav[p + 1], fav[p + 2], fav[p + 3]]) as usize;
            if n < 2 || p + 4 + n > fav.len() {
                break;
            }
            let mut idl = fav[p + 4..p + 4 + n].to_vec();
            idl.extend_from_slice(&[0, 0, 0, 0]);
            p += 4 + n + 1;
            let Ok(item) = SHCreateItemFromIDList::<IShellItem>(idl.as_ptr() as *const windows::Win32::UI::Shell::Common::ITEMIDLIST) else { continue };
            let get = |k: SIGDN| item.GetDisplayName(k).ok().map(|s| {
                let r = s.to_string().unwrap_or_default();
                windows::Win32::System::Com::CoTaskMemFree(Some(s.0 as *const _));
                r
            });
            if let Some(path) = get(SIGDN_FILESYSPATH).filter(|p| p.to_lowercase().ends_with(".lnk") && std::path::Path::new(p).exists()) {
                out.push(pin_from_lnk(&path, size));
            } else if let Some(aumid) = get(SIGDN_PARENTRELATIVEPARSING).filter(|a| a.contains('!') || !a.contains('\\')) {
                let name = get(SIGDN_NORMALDISPLAY).unwrap_or_else(|| aumid.clone());
                let icon = store_logo(&item, size).or_else(|| logo_in_idlist(&idl, size)).or_else(|| aumid_icon(&aumid, size)).or_else(|| item_icon(&item, size));
                out.push(Pin { lnk: format!(r"shell:AppsFolder\{aumid}"), target: String::new(), name, icon, aumid });
            }
        }
    }
    (!out.is_empty()).then_some(out)
}

/// Đọc thứ tự các ô app trên taskbar gốc của Windows (qua UI Automation): danh sách ID app
/// ("Appid: ..." của từng ô, từ trái sang phải). Chỉ đọc được khi taskbar gốc đang hiện.
fn read_native_order() -> Option<Vec<String>> {
    use windows::Win32::System::Com::*;
    use windows::Win32::UI::Accessibility::*;
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let ua: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
        // taskbar gốc đang hiện: màn chính trước, rồi các màn phụ (bỏ qua khay riêng của app)
        let mut bars = Vec::new();
        let mine = own_tray();
        let mut prev = HWND::default();
        while let Ok(h) = FindWindowExW(HWND::default(), prev, w!("Shell_TrayWnd"), PCWSTR::null()) {
            if h != mine && IsWindowVisible(h).as_bool() {
                bars.push(h);
            }
            prev = h;
        }
        let mut prev = HWND::default();
        while let Ok(h) = FindWindowExW(HWND::default(), prev, w!("Shell_SecondaryTrayWnd"), PCWSTR::null()) {
            if IsWindowVisible(h).as_bool() {
                bars.push(h);
            }
            prev = h;
        }
        for h in bars {
            let Ok(root) = ua.ElementFromHandle(h) else { continue };
            let Ok(all) = root.FindAll(TreeScope_Descendants, &ua.CreateTrueCondition().ok()?) else { continue };
            let n = all.Length().unwrap_or(0);
            let mut items: Vec<(i32, String)> = Vec::new();
            for i in 0..n {
                let Ok(e) = all.GetElement(i) else { continue };
                let cls = e.CurrentClassName().map(|b| b.to_string()).unwrap_or_default();
                if cls != "Taskbar.TaskListButtonAutomationPeer" {
                    continue;
                }
                let id = e.CurrentAutomationId().map(|b| b.to_string()).unwrap_or_default();
                let Some(id) = id.strip_prefix("Appid: ") else { continue };
                let x = e.CurrentBoundingRectangle().map(|r| r.left).unwrap_or(0);
                items.push((x, id.to_lowercase()));
            }
            if !items.is_empty() {
                items.sort_by_key(|i| i.0);
                return Some(items.into_iter().map(|i| i.1).collect());
            }
        }
        None
    }
}

/// Cửa sổ "Shell_TrayWnd" của chính app (khay riêng) – không nhầm với taskbar gốc.
fn own_tray() -> HWND {
    let mut h = HWND::default();
    unsafe {
        let mut prev = HWND::default();
        while let Ok(t) = FindWindowExW(HWND::default(), prev, w!("Shell_TrayWnd"), PCWSTR::null()) {
            let mut pid = 0u32;
            GetWindowThreadProcessId(t, Some(&mut pid));
            if pid == std::process::id() {
                h = t;
            }
            prev = t;
        }
    }
    h
}

/// Thứ tự ô app của taskbar gốc (số lần đổi, các ID app), đọc ở luồng nền khi taskbar gốc đang
/// hiện; lưu ra file để lần sau (khi taskbar gốc đã bị ẩn) vẫn xếp đúng.
static NATIVE_ORDER: std::sync::Mutex<(u64, Vec<String>)> = std::sync::Mutex::new((0, Vec::new()));

pub fn native_order() -> (u64, Vec<String>) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let file = crate::config::dir().join("taskbar-order.txt");
        if let Ok(s) = std::fs::read_to_string(&file) {
            let v: Vec<String> = s.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
            if let Ok(mut o) = NATIVE_ORDER.lock() {
                *o = (1, v);
            }
        }
        let _ = std::thread::Builder::new().name("thu-tu-taskbar".into()).spawn(move || {
            // chỉ đổi theo taskbar gốc khi chính nó đổi (không đè thứ tự vừa kéo trên taskbar app)
            let mut last: Option<Vec<String>> = None;
            loop {
                if let Some(v) = read_native_order() {
                    if last.as_ref() != Some(&v) {
                        set_native_order(v.clone());
                        last = Some(v);
                        // báo taskbar của app vẽ lại ngay
                        APPS_CHANGED.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(700));
            }
        });
    });
    NATIVE_ORDER.lock().map(|o| o.clone()).unwrap_or_default()
}

/// Đặt thứ tự chung của các ô app (ID app, chữ thường): từ taskbar gốc, hoặc sau khi kéo đổi chỗ
/// trên taskbar của app. Lưu ra file để lần mở app sau vẫn đúng.
pub fn set_native_order(v: Vec<String>) {
    if let Ok(mut o) = NATIVE_ORDER.lock() {
        if o.1 != v {
            o.0 += 1;
            let _ = std::fs::write(crate::config::dir().join("taskbar-order.txt"), v.join("\n"));
            o.1 = v;
        }
    }
}

/// Thứ tự ô app mới nhất sau khi kéo đổi chỗ ở một màn hình (số lần đổi, khoá các ô), để taskbar
/// ở màn hình khác xếp theo.
static TILE_ORDER: std::sync::Mutex<(u64, Vec<String>)> = std::sync::Mutex::new((0, Vec::new()));

pub fn publish_tile_order(keys: Vec<String>) {
    if let Ok(mut o) = TILE_ORDER.lock() {
        o.0 += 1;
        o.1 = keys;
    }
}

pub fn tile_order() -> (u64, Vec<String>) {
    TILE_ORDER.lock().map(|o| o.clone()).unwrap_or_default()
}

/// "Dấu vân tay" rẻ của danh sách ghim (đọc registry + thư mục), để chỉ đọc lại toàn bộ
/// (tốn vài chục ms vì phải lấy icon) khi danh sách thật sự đổi.
pub fn pins_signature() -> Vec<u8> {
    reg_get(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Taskband", "Favorites").unwrap_or_default()
}

/// Trạng thái mạng, cập nhật ở luồng riêng mỗi 2 giây (không làm chậm luồng vẽ).
pub fn network_cached() -> Net {
    use std::sync::atomic::{AtomicU8, Ordering};
    static NET: AtomicU8 = AtomicU8::new(255);
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let _ = std::thread::Builder::new().name("mang".into()).spawn(|| loop {
            let n = match network() {
                Net::Wifi => 0,
                Net::Ethernet => 1,
                Net::Offline => 2,
            };
            NET.store(n, Ordering::Relaxed);
            std::thread::sleep(std::time::Duration::from_secs(2));
        });
    });
    match NET.load(Ordering::Relaxed) {
        0 => Net::Wifi,
        2 => Net::Offline,
        _ => Net::Ethernet,
    }
}

/// Các app đã ghim trên taskbar Windows, đúng thứ tự như taskbar gốc (cả app Store, lối tắt hồ sơ
/// Chrome...). Chỉ lấy từ Windows để taskbar của app và taskbar gốc luôn giống nhau.
pub fn pinned_apps(size: i32) -> Vec<Pin> {
    favorites(size).unwrap_or_else(|| {
        // Không đọc được danh sách của Windows: dùng thư mục lối tắt ghim
        let dir = std::env::var_os("APPDATA").map(|a| std::path::PathBuf::from(a).join(r"Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar"));
        let mut v: Vec<Pin> = dir
            .and_then(|d| std::fs::read_dir(d).ok())
            .map(|r| r.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("lnk"))).map(|p| pin_from_lnk(&p.to_string_lossy(), size)).collect())
            .unwrap_or_default();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v
    })
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Mở danh sách mạng qua Shell để tránh đưa focus bàn phím vào nút bật/tắt Wi-Fi.
pub fn open_network_flyout() {
    let result = unsafe {
        ShellExecuteW(HWND::default(), w!("open"), w!("ms-availablenetworks:"),
            PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL)
    };
    if result.0 as isize <= 32 {
        open("ms-settings:network-status");
    }
}

/// Mở một trang Cài đặt của Windows hoặc chương trình (ví dụ "ms-settings:sound", "taskmgr").
pub fn open(target: &str) {
    launch(target);
}

/// Đóng một cửa sổ (như bấm X).
pub fn close_window(hwnd: isize) {
    unsafe {
        let _ = PostMessageW(HWND(hwnd as *mut _), WM_CLOSE, WPARAM(0), LPARAM(0));
    }
}

/// Mở app đã ghim (như bấm vào nó trên taskbar gốc).
pub fn launch(lnk: &str) {
    let w: Vec<u16> = lnk.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        ShellExecuteW(HWND::default(), w!("open"), PCWSTR(w.as_ptr()), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL);
    }
}

// ------------------------------------------------------------------ âm lượng

/// Âm lượng loa mặc định (Core Audio), giống nút loa của taskbar gốc.
pub struct Audio {
    ep: windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume,
    en: windows::Win32::Media::Audio::IMMDeviceEnumerator,
    /// Mã thiết bị phát mặc định lúc lấy `ep` (đổi loa/tai nghe thì phải lấy lại)
    id: String,
}

impl Audio {
    pub fn new() -> Option<Self> {
        use windows::Win32::Media::Audio::*;
        use windows::Win32::System::Com::*;
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let en: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
            let dev = en.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
            let id = Self::device_id(&dev);
            let ep = dev.Activate(CLSCTX_ALL, None).ok()?;
            Some(Self { ep, en, id })
        }
    }

    unsafe fn device_id(dev: &windows::Win32::Media::Audio::IMMDevice) -> String {
        match dev.GetId() {
            Ok(p) => {
                let s = p.to_string().unwrap_or_default();
                windows::Win32::System::Com::CoTaskMemFree(Some(p.0 as *const _));
                s
            }
            Err(_) => String::new(),
        }
    }

    /// Tăng/giảm âm lượng trực tiếp (bước 2% như cuộn chuột trên loa của taskbar gốc), không hiện
    /// bảng âm lượng của Windows. Tăng khi đang tắt tiếng thì bật tiếng lại.
    pub fn step(&self, d: f32) -> (f32, bool) {
        let (v, muted) = self.get();
        let nv = (((v + d) * 50.0).round() / 50.0).clamp(0.0, 1.0);
        unsafe {
            let _ = self.ep.SetMasterVolumeLevelScalar(nv, std::ptr::null());
            if muted && d > 0.0 {
                let _ = self.ep.SetMute(false, std::ptr::null());
            }
        }
        self.get()
    }

    /// Thiết bị phát mặc định đã đổi (cắm tai nghe, chọn loa khác…)?
    pub fn device_changed(&self) -> bool {
        use windows::Win32::Media::Audio::*;
        unsafe {
            match self.en.GetDefaultAudioEndpoint(eRender, eConsole) {
                Ok(dev) => Self::device_id(&dev) != self.id,
                Err(_) => !self.id.is_empty(),
            }
        }
    }

    /// (mức 0..1, đang tắt tiếng)
    pub fn get(&self) -> (f32, bool) {
        unsafe { (self.ep.GetMasterVolumeLevelScalar().unwrap_or(0.0), self.ep.GetMute().map(|b| b.as_bool()).unwrap_or(false)) }
    }
}
