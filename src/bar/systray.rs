//! Khay hệ thống riêng của app.
//!
//! Các app thêm icon vào khay bằng Shell_NotifyIcon: Windows tìm cửa sổ lớp "Shell_TrayWnd"
//! rồi gửi WM_COPYDATA. App tạo một cửa sổ cùng lớp nằm trên cùng để nhận các tin này,
//! ghi lại icon/tên/cách gọi lại, rồi CHUYỂN TIẾP NGUYÊN VẸN sang taskbar thật của Windows
//! (nên Windows vẫn đầy đủ như cũ). Nhờ vậy taskbar của app hiện icon thật và bấm trái/phải
//! vào icon hoạt động y như khay gốc, không cần mở taskbar gốc.
//!
//! Khi app tắt (kể cả tắt đột ngột), cửa sổ này biến mất và mọi thứ quay về Windows.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::System::DataExchange::COPYDATASTRUCT;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

use super::shell;

/// NOTIFYICONDATA dạng 32-bit mà shell đóng gói khi gửi cho taskbar.
#[repr(C)]
#[derive(Clone, Copy)]
struct Nid32 {
    cb: u32,
    hwnd: u32,
    uid: u32,
    flags: u32,
    callback: u32,
    icon: u32,
    tip: [u16; 128],
    state: u32,
    state_mask: u32,
    info: [u16; 256],
    version: u32,
    info_title: [u16; 64],
    info_flags: u32,
    guid: [u8; 16],
    balloon_icon: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct TrayData {
    signature: u32,
    message: u32,
    nid: Nid32,
}

const NIF_MESSAGE: u32 = 0x1;
const NIF_ICON: u32 = 0x2;
const NIF_TIP: u32 = 0x4;
const NIF_STATE: u32 = 0x8;
const NIF_GUID: u32 = 0x20;
const NIS_HIDDEN: u32 = 0x1;

#[derive(Clone)]
pub struct TrayIcon {
    pub hwnd: isize,
    pub uid: u32,
    pub guid: Option<[u8; 16]>,
    pub callback: u32,
    pub version: u32,
    pub tip: String,
    /// Ảnh icon 32x32 RGBA (đã nhân alpha)
    pub rgba: Option<Vec<u8>>,
    pub hidden: bool,
    /// Tên file chương trình (chữ thường)
    pub exe: String,
}

static ICONS: Mutex<Vec<TrayIcon>> = Mutex::new(Vec::new());
static GEN: AtomicU64 = AtomicU64::new(0);

/// GUID (phần đầu) của các icon hệ thống Windows vẽ riêng cạnh đồng hồ:
/// âm lượng, mạng, pin. Thanh của app đã có các nút này nên không lặp lại.
const SYSTEM_ICONS: [u32; 3] = [0x7820_AE73, 0x7820_AE74, 0x7820_AE75];

fn is_system_icon(t: &TrayIcon) -> bool {
    t.guid.map(|g| SYSTEM_ICONS.contains(&u32::from_le_bytes([g[0], g[1], g[2], g[3]]))).unwrap_or(false)
}

/// Danh sách icon hiện tại (bỏ các icon hệ thống đã có trên thanh).
pub fn icons() -> Vec<TrayIcon> {
    ICONS.lock().map(|v| v.iter().filter(|t| !is_system_icon(t)).cloned().collect()).unwrap_or_default()
}

/// Tăng mỗi khi khay thay đổi (để biết lúc nào cần vẽ lại).
pub fn generation() -> u64 {
    GEN.load(Ordering::Relaxed)
}

fn wstr(b: &[u16]) -> String {
    let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf16_lossy(&b[..n])
}

fn handle(td: &TrayData) {
    let n = &td.nid;
    let hwnd = n.hwnd as i32 as isize;
    let guid = if n.flags & NIF_GUID != 0 { Some(n.guid) } else { None };
    let Ok(mut list) = ICONS.lock() else { return };
    let pos = list.iter().position(|t| match (guid, t.guid) {
        (Some(g), Some(tg)) => g == tg,
        _ => t.hwnd == hwnd && t.uid == n.uid,
    });
    match td.message {
        // NIM_ADD, NIM_MODIFY
        0 | 1 => {
            let i = match pos {
                Some(i) => i,
                None => {
                    let exe = shell::exe_name(hwnd);
                    list.push(TrayIcon { hwnd, uid: n.uid, guid, callback: 0, version: 0, tip: String::new(), rgba: None, hidden: false, exe });
                    list.len() - 1
                }
            };
            let t = &mut list[i];
            t.hwnd = hwnd;
            if n.flags & NIF_MESSAGE != 0 {
                t.callback = n.callback;
            }
            if n.flags & NIF_ICON != 0 && n.icon != 0 {
                t.rgba = shell::icon_rgba(HICON(n.icon as i32 as isize as *mut _), 32);
            }
            if n.flags & NIF_TIP != 0 {
                t.tip = wstr(&n.tip);
            }
            if n.flags & NIF_STATE != 0 && n.state_mask & NIS_HIDDEN != 0 {
                t.hidden = n.state & NIS_HIDDEN != 0;
            }
        }
        // NIM_DELETE
        2 => {
            if let Some(i) = pos {
                list.remove(i);
            }
        }
        // NIM_SETVERSION
        4 => {
            if let Some(i) = pos {
                list[i].version = n.version;
            }
        }
        _ => {}
    }
    GEN.fetch_add(1, Ordering::Relaxed);
    // Ghi danh sách ra file khi cần kiểm tra (đặt biến môi trường SEASON_TRAY_DUMP=1)
    if std::env::var_os("SEASON_TRAY_DUMP").is_none() {
        return;
    }
    let mut out = String::new();
    for t in list.iter() {
        let g = t.guid.map(|g| format!("{:02x}{:02x}{:02x}{:02x}", g[3], g[2], g[1], g[0])).unwrap_or_default();
        out += &format!("{} | ẩn={} | guid={} | msg={:#x} | {}\n", t.exe, t.hidden, g, t.callback, t.tip.replace('\n', " / "));
    }
    let _ = std::fs::write(crate::config::dir().join("tray-dump.txt"), out);
}

unsafe extern "system" fn wndproc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_COPYDATA && lp.0 != 0 {
        let cds = &*(lp.0 as *const COPYDATASTRUCT);
        if cds.dwData == 1 && !cds.lpData.is_null() && cds.cbData as usize >= 8 + 24 {
            // Chép vào vùng nhớ đủ lớn (app cũ có thể gửi gói nhỏ hơn).
            let mut td: TrayData = std::mem::zeroed();
            let n = (cds.cbData as usize).min(std::mem::size_of::<TrayData>());
            std::ptr::copy_nonoverlapping(cds.lpData as *const u8, &mut td as *mut TrayData as *mut u8, n);
            if td.signature == 0x3475_3423 {
                handle(&td);
            }
        }
    }
    // Mọi tin nhắn dành cho taskbar đều được chuyển tiếp sang taskbar thật.
    if msg == WM_COPYDATA || msg == WM_COMMAND || msg >= WM_USER {
        if let Some(real) = shell::real_tray() {
            let mut res = 0usize;
            let _ = SendMessageTimeoutW(real, msg, wp, lp, SMTO_ABORTIFHUNG, 1500, Some(&mut res));
            return LRESULT(res as isize);
        }
        return LRESULT(0);
    }
    DefWindowProcW(h, msg, wp, lp)
}

pub struct Host {
    hwnd: HWND,
}

fn broadcast_taskbar_created() {
    unsafe {
        let m = RegisterWindowMessageW(w!("TaskbarCreated"));
        if m == 0 {
            return;
        }
        // Gửi riêng cho từng cửa sổ của app KHÁC (không phát chung): cửa sổ của chính app
        // mà nhận tin này thì thư viện giao diện sẽ đặt lại kiểu cửa sổ, làm hỏng taskbar.
        let me = std::process::id();
        unsafe extern "system" fn each(h: HWND, lp: LPARAM) -> BOOL {
            let (me, m) = *(lp.0 as *const (u32, u32));
            let mut pid = 0u32;
            GetWindowThreadProcessId(h, Some(&mut pid));
            if pid != me {
                let _ = PostMessageW(h, m, WPARAM(0), LPARAM(0));
            }
            TRUE
        }
        let data = (me, m);
        let _ = EnumWindows(Some(each), LPARAM(&data as *const (u32, u32) as isize));
        // Cửa sổ "chỉ nhận tin nhắn" của các app (trừ app này và Explorer).
        let mut explorer = 0u32;
        if let Some(t) = shell::real_tray() {
            GetWindowThreadProcessId(t, Some(&mut explorer));
        }
        let mut prev = HWND::default();
        while let Ok(h) = FindWindowExW(HWND_MESSAGE, prev, PCWSTR::null(), PCWSTR::null()) {
            let mut pid = 0u32;
            GetWindowThreadProcessId(h, Some(&mut pid));
            if pid != me && pid != explorer {
                let _ = PostMessageW(h, m, WPARAM(0), LPARAM(0));
            }
            prev = h;
        }
    }
}

impl Host {
    pub fn start() -> Option<Host> {
        unsafe {
            let inst = GetModuleHandleW(PCWSTR::null()).ok()?;
            let wc = WNDCLASSW { lpfnWndProc: Some(wndproc), hInstance: inst.into(), lpszClassName: w!("Shell_TrayWnd"), ..Default::default() };
            RegisterClassW(&wc);
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                w!("Shell_TrayWnd"),
                w!(""),
                WS_POPUP,
                0,
                0,
                0,
                0,
                HWND::default(),
                HMENU::default(),
                inst,
                None,
            )
            .ok()?;
            let host = Host { hwnd };
            host.keep_on_top();
            // Nhờ các app gửi lại icon khay của chúng (giống lúc Explorer khởi động lại).
            broadcast_taskbar_created();
            std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_secs(2));
                broadcast_taskbar_created();
            });
            crate::log("đã bật khay hệ thống riêng");
            Some(host)
        }
    }

    /// Giữ cửa sổ nằm trên taskbar thật để các app tìm thấy nó trước.
    pub fn keep_on_top(&self) {
        unsafe {
            let _ = SetWindowPos(self.hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
    }

    #[allow(dead_code)]
    pub fn hwnd(&self) -> isize {
        self.hwnd.0 as isize
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
        if let Ok(mut l) = ICONS.lock() {
            l.clear();
        }
        // Các app gửi lại icon cho taskbar gốc cho chắc.
        broadcast_taskbar_created();
        crate::log("đã tắt khay hệ thống riêng");
    }
}

/// Bấm vào icon khay: gửi đúng các tin nhắn mà taskbar gốc gửi cho app.
/// (x, y) là toạ độ màn hình của icon, để menu của app hiện đúng chỗ.
pub fn click(t: &TrayIcon, right: bool, x: i32, y: i32) {
    crate::log(&format!("khay: bấm {} vào {} (cửa sổ {:#x}, tin {:#x}, phiên bản {})", if right { "phải" } else { "trái" }, t.exe, t.hwnd, t.callback, t.version));
    // Icon thay thế (app không gửi icon sang taskbar của app): mở đúng ứng dụng của nó
    if t.hwnd == 0 {
        match t.exe.as_str() {
            "nvdisplay.container.exe" => shell::launch(r"shell:AppsFolder\NVIDIACorp.NVIDIAControlPanel_56jybvy8sckqj!NVIDIACorp.NVIDIAControlPanel"),
            // icon của chính app (khi chưa kịp gửi icon thật): mở cửa sổ cài đặt
            "season-desktop.exe" => {
                if let Ok(me) = std::env::current_exe() {
                    let _ = std::process::Command::new(me).spawn();
                }
            }
            _ => {}
        }
        return;
    }
    let _ = (x, y);
    if t.callback == 0 {
        // Một số icon hệ thống không nhận tin nhắn trực tiếp: mở đúng ứng dụng của nó.
        if t.exe == "securityhealthsystray.exe" {
            let _ = std::process::Command::new("explorer.exe").arg("windowsdefender:").spawn();
        }
        return;
    }
    let h = HWND(t.hwnd as *mut _);
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(h, Some(&mut pid));
        // Như taskbar gốc: cho app quyền đưa cửa sổ (menu) của nó lên trước. App vừa nhận
        // cú bấm là app này nên được quyền cấp. Với chuột phải, đưa luôn cửa sổ nhận tin của
        // app lên trước: menu ngữ cảnh (TrackPopupMenu) chỉ hiện và giữ được khi app đang ở trước.
        let _ = AllowSetForegroundWindow(pid);
        if right {
            let _ = SetForegroundWindow(h);
        }
        let post = |wp: usize, lp: isize| {
            let _ = PostMessageW(h, t.callback, WPARAM(wp), LPARAM(lp));
        };
        const NIN_SELECT: u32 = WM_USER;
        if t.version >= 4 {
            let wp = (x as u16 as usize) | ((y as u16 as usize) << 16);
            let ev = |m: u32| ((m & 0xFFFF) | (t.uid << 16)) as i32 as isize;
            if right {
                post(wp, ev(WM_RBUTTONDOWN));
                post(wp, ev(WM_RBUTTONUP));
                post(wp, ev(WM_CONTEXTMENU));
            } else {
                post(wp, ev(WM_LBUTTONDOWN));
                post(wp, ev(WM_LBUTTONUP));
                post(wp, ev(NIN_SELECT));
            }
        } else {
            let wp = t.uid as usize;
            if right {
                post(wp, WM_RBUTTONDOWN as isize);
                post(wp, WM_RBUTTONUP as isize);
                if t.version >= 3 {
                    post(wp, WM_CONTEXTMENU as isize);
                }
            } else {
                post(wp, WM_LBUTTONDOWN as isize);
                post(wp, WM_LBUTTONUP as isize);
                if t.version >= 3 {
                    post(wp, NIN_SELECT as isize);
                }
            }
        }
    }
}

/// Bấm đúp (nhiều app mở cửa sổ chính khi bấm đúp icon khay).
#[allow(dead_code)]
pub fn double_click(t: &TrayIcon, x: i32, y: i32) {
    click(t, false, x, y);
    let h = HWND(t.hwnd as *mut _);
    unsafe {
        let (wp, lp) = if t.version >= 4 {
            ((x as u16 as usize) | ((y as u16 as usize) << 16), ((WM_LBUTTONDBLCLK & 0xFFFF) | (t.uid << 16)) as i32 as isize)
        } else {
            (t.uid as usize, WM_LBUTTONDBLCLK as isize)
        };
        let _ = PostMessageW(h, t.callback, WPARAM(wp), LPARAM(lp));
    }
}
