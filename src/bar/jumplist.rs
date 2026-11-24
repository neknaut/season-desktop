//! Jump list của app (giống menu chuột phải của taskbar gốc): các mục "Tasks" và nhóm riêng
//! mà app tự khai báo (file .customDestinations-ms của Windows), cùng mục "Gần đây" (Recent).

use windows::core::{Interface, GUID, PCWSTR, PWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::StructuredStorage::*;
use windows::Win32::System::Com::*;
use windows::Win32::UI::Shell::Common::IObjectArray;
use windows::Win32::UI::Shell::PropertiesSystem::*;
use windows::Win32::UI::Shell::*;

/// Một mục trong jump list.
pub struct Entry {
    pub title: String,
    /// Chương trình hoặc file cần mở
    pub path: String,
    pub args: String,
    pub dir: String,
    /// Nơi lấy icon: (file, chỉ số)
    pub icon: Option<(String, i32)>,
}

/// Một nhóm (ví dụ "Tasks", "Recent", "Most visited").
pub struct Group {
    pub name: String,
    pub entries: Vec<Entry>,
}

const PKEY_TITLE: PROPERTYKEY = PROPERTYKEY { fmtid: GUID::from_u128(0xF29F85E0_4FF9_1068_AB91_08002B27B3D9), pid: 2 };
const PKEY_AUMID: PROPERTYKEY = PROPERTYKEY { fmtid: GUID::from_u128(0x9F4C2855_9F79_4B39_A8D0_E1D42DE1D5F3), pid: 5 };
const PKEY_SEPARATOR: PROPERTYKEY = PROPERTYKEY { fmtid: GUID::from_u128(0x9F4C2855_9F79_4B39_A8D0_E1D42DE1D5F3), pid: 6 };

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn com() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
}

fn prop_string(ps: &IPropertyStore, key: &PROPERTYKEY) -> Option<String> {
    unsafe {
        let v = ps.GetValue(key).ok()?;
        let p = PropVariantToStringAlloc(&v).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const _));
        s.filter(|s| !s.is_empty())
    }
}

fn prop_bool(ps: &IPropertyStore, key: &PROPERTYKEY) -> bool {
    unsafe { ps.GetValue(key).ok().and_then(|v| PropVariantToBoolean(&v).ok()).map(|b| b.as_bool()).unwrap_or(false) }
}

fn buf_string(buf: &[u16]) -> String {
    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..n])
}

/// Đọc một lối tắt (.lnk) đã nằm trong bộ nhớ của file jump list. Trả về mục và số byte đã đọc.
fn read_link(data: &[u8]) -> Option<(Option<Entry>, usize)> {
    unsafe {
        let stream = SHCreateMemStream(Some(data))?;
        let sl: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let ps: IPersistStream = sl.cast().ok()?;
        ps.Load(&stream).ok()?;
        let mut pos = 0u64;
        stream.Seek(0, STREAM_SEEK_CUR, Some(&mut pos)).ok()?;
        let store: Option<IPropertyStore> = sl.cast().ok();
        if store.as_ref().is_some_and(|s| prop_bool(s, &PKEY_SEPARATOR)) {
            return Some((None, pos as usize));
        }
        let mut path = [0u16; 1024];
        let _ = sl.GetPath(&mut path, std::ptr::null_mut(), 0);
        let mut args = [0u16; 2048];
        let _ = sl.GetArguments(&mut args);
        let mut dir = [0u16; 1024];
        let _ = sl.GetWorkingDirectory(&mut dir);
        let mut ipath = [0u16; 1024];
        let mut iidx = 0i32;
        let _ = sl.GetIconLocation(&mut ipath, &mut iidx);
        let mut title = store.as_ref().and_then(|s| prop_string(s, &PKEY_TITLE)).unwrap_or_default();
        if title.is_empty() {
            let mut d = [0u16; 1024];
            let _ = sl.GetDescription(&mut d);
            title = buf_string(&d);
        }
        // Tiêu đề dạng "@file.dll,-123" (chuỗi trong tài nguyên của chương trình)
        if title.starts_with('@') {
            let mut out = [0u16; 512];
            let w = wide(&title);
            if SHLoadIndirectString(PCWSTR(w.as_ptr()), &mut out, None).is_ok() {
                title = buf_string(&out);
            }
        }
        let path = buf_string(&path);
        if title.is_empty() || path.is_empty() {
            return Some((None, pos as usize));
        }
        let ipath = expand(&buf_string(&ipath));
        let icon = if ipath.is_empty() { Some((path.clone(), 0)) } else { Some((ipath, iidx)) };
        Some((Some(Entry { title, path, args: buf_string(&args), dir: buf_string(&dir), icon }), pos as usize))
    }
}

fn expand(s: &str) -> String {
    if !s.contains('%') {
        return s.to_string();
    }
    let w = wide(s);
    let mut out = vec![0u16; 2048];
    let n = unsafe { windows::Win32::System::Environment::ExpandEnvironmentStringsW(PCWSTR(w.as_ptr()), Some(&mut out)) };
    if n == 0 {
        return s.to_string();
    }
    buf_string(&out)
}

const LNK_CLSID: [u8; 16] = [0x01, 0x14, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46];
const FOOTER: u32 = 0xBABF_FBAB;

enum Cat {
    Custom(String, Vec<Entry>),
    Known(i32),
    Tasks(Vec<Entry>),
}

fn u32_at(b: &[u8], p: usize) -> Option<u32> {
    b.get(p..p + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn read_entries(b: &[u8], p: &mut usize, n: u32) -> Option<Vec<Entry>> {
    let mut out = Vec::new();
    for _ in 0..n.min(200) {
        if b.get(*p..*p + 16)? != LNK_CLSID {
            return None;
        }
        *p += 16;
        let (e, used) = read_link(b.get(*p..)?)?;
        *p += used;
        if let Some(e) = e {
            out.push(e);
        }
    }
    Some(out)
}

/// Đọc file .customDestinations-ms (định dạng của Windows: các nhóm, mỗi nhóm là danh sách lối tắt).
fn parse_custom(b: &[u8]) -> Option<Vec<Cat>> {
    if u32_at(b, 0)? != 2 {
        return None;
    }
    let ncat = u32_at(b, 4)?;
    let mut p = 12usize;
    let mut cats = Vec::new();
    for _ in 0..ncat.min(20) {
        let kind = u32_at(b, p)?;
        p += 4;
        match kind {
            0 => {
                let len = b.get(p..p + 2).map(|s| u16::from_le_bytes([s[0], s[1]]))? as usize;
                p += 2;
                let name: Vec<u16> = b.get(p..p + len * 2)?.chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
                p += len * 2;
                let n = u32_at(b, p)?;
                p += 4;
                let entries = read_entries(b, &mut p, n)?;
                let mut name = String::from_utf16_lossy(&name);
                if name.starts_with('@') {
                    let mut out = [0u16; 256];
                    let w = wide(&name);
                    if unsafe { SHLoadIndirectString(PCWSTR(w.as_ptr()), &mut out, None) }.is_ok() {
                        name = buf_string(&out);
                    }
                }
                cats.push(Cat::Custom(name, entries));
            }
            1 => {
                let id = u32_at(b, p)? as i32;
                p += 4;
                cats.push(Cat::Known(id));
            }
            2 => {
                let n = u32_at(b, p)?;
                p += 4;
                cats.push(Cat::Tasks(read_entries(b, &mut p, n)?));
            }
            _ => return None,
        }
        if u32_at(b, p)? != FOOTER {
            return None;
        }
        p += 4;
    }
    Some(cats)
}

/// AppUserModelID của cửa sổ hoặc lối tắt (để lấy danh sách "Gần đây" của đúng app).
fn aumid(hwnd: Option<isize>, lnk: Option<&str>, exe: &str) -> String {
    unsafe {
        if let Some(h) = hwnd.filter(|h| *h > 0) {
            if let Ok(ps) = SHGetPropertyStoreForWindow::<_, IPropertyStore>(HWND(h as *mut _)) {
                if let Some(s) = prop_string(&ps, &PKEY_AUMID) {
                    return s;
                }
            }
        }
        if let Some(l) = lnk {
            if let Ok(sl) = CoCreateInstance::<_, IShellLinkW>(&ShellLink, None, CLSCTX_INPROC_SERVER) {
                if let Ok(pf) = sl.cast::<IPersistFile>() {
                    let w = wide(l);
                    if pf.Load(PCWSTR(w.as_ptr()), STGM_READ).is_ok() {
                        if let Some(s) = sl.cast::<IPropertyStore>().ok().and_then(|ps| prop_string(&ps, &PKEY_AUMID)) {
                            return s;
                        }
                    }
                }
            }
        }
    }
    // App thường không tự đặt ID: Windows dùng đường dẫn chương trình (thư mục quen thuộc viết bằng GUID).
    let low = exe.to_lowercase();
    let folders = [
        (std::env::var("ProgramFiles").unwrap_or_default(), "{6D809377-6AF0-444B-8957-A3773F02200E}"),
        (std::env::var("ProgramFiles(x86)").unwrap_or_default(), "{7C5A40EF-A0FB-4BFC-874A-C0F2E0B9FA8E}"),
        (std::env::var("SystemRoot").map(|s| s + r"\System32").unwrap_or_default(), "{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}"),
        (std::env::var("SystemRoot").unwrap_or_default(), "{F38BF404-1D43-42F2-9305-67DE0B28FC23}"),
    ];
    for (dir, guid) in folders {
        if !dir.is_empty() && low.starts_with(&(dir.to_lowercase() + "\\")) {
            return format!("{guid}{}", &exe[dir.len()..]);
        }
    }
    exe.to_string()
}

const PKEY_RELAUNCH_ICON: PROPERTYKEY = PROPERTYKEY { fmtid: GUID::from_u128(0x9F4C2855_9F79_4B39_A8D0_E1D42DE1D5F3), pid: 3 };

/// AppUserModelID mà cửa sổ khai báo (rỗng nếu không có).
pub fn window_aumid(h: isize) -> String {
    unsafe {
        SHGetPropertyStoreForWindow::<_, IPropertyStore>(HWND(h as *mut _)).ok().and_then(|ps| prop_string(&ps, &PKEY_AUMID)).unwrap_or_default()
    }
}

/// Icon riêng mà cửa sổ khai báo cho taskbar (ví dụ ảnh hồ sơ của Chrome): (file, chỉ số).
pub fn relaunch_icon(h: isize) -> Option<(String, i32)> {
    let s = unsafe { SHGetPropertyStoreForWindow::<_, IPropertyStore>(HWND(h as *mut _)).ok().and_then(|ps| prop_string(&ps, &PKEY_RELAUNCH_ICON))? };
    let (p, i) = match s.rsplit_once(',') {
        Some((p, i)) if i.trim().parse::<i32>().is_ok() => (p.to_string(), i.trim().parse::<i32>().unwrap_or(0)),
        _ => (s.clone(), 0),
    };
    let p = expand(p.trim().trim_matches('"'));
    std::path::Path::new(&p).exists().then_some((p, i))
}

/// AppUserModelID ghi trong một lối tắt.
pub fn lnk_aumid(lnk: &str) -> Option<String> {
    com();
    unsafe {
        let sl: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let pf: IPersistFile = sl.cast().ok()?;
        let w = wide(lnk);
        pf.Load(PCWSTR(w.as_ptr()), STGM_READ).ok()?;
        prop_string(&sl.cast::<IPropertyStore>().ok()?, &PKEY_AUMID)
    }
}

/// Danh sách "Gần đây" (-2) hoặc "Thường dùng" (-1) mà Windows lưu cho app.
fn known_list(id: &str, frequent: bool) -> Vec<Entry> {
    let mut out = Vec::new();
    unsafe {
        let Ok(adl) = CoCreateInstance::<_, IApplicationDocumentLists>(&ApplicationDocumentLists, None, CLSCTX_INPROC_SERVER) else { return out };
        let w = wide(id);
        if adl.SetAppID(PCWSTR(w.as_ptr())).is_err() {
            return out;
        }
        let Ok(arr) = adl.GetList::<IObjectArray>(if frequent { ADLT_FREQUENT } else { ADLT_RECENT }, 10) else { return out };
        let n = arr.GetCount().unwrap_or(0);
        for i in 0..n {
            if let Ok(item) = arr.GetAt::<IShellItem>(i) {
                let name = item.GetDisplayName(SIGDN_NORMALDISPLAY).ok().map(|p| take(p)).unwrap_or_default();
                let path = item.GetDisplayName(SIGDN_DESKTOPABSOLUTEPARSING).ok().map(|p| take(p)).unwrap_or_default();
                if !name.is_empty() && !path.is_empty() {
                    out.push(Entry { title: name, icon: Some((path.clone(), 0)), path, args: String::new(), dir: String::new() });
                }
            } else if let Ok(sl) = arr.GetAt::<IShellLinkW>(i) {
                let mut p = [0u16; 1024];
                let _ = sl.GetPath(&mut p, std::ptr::null_mut(), 0);
                let path = buf_string(&p);
                let title = sl.cast::<IPropertyStore>().ok().and_then(|ps| prop_string(&ps, &PKEY_TITLE)).unwrap_or_else(|| {
                    std::path::Path::new(&path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
                });
                if !path.is_empty() {
                    let mut a = [0u16; 2048];
                    let _ = sl.GetArguments(&mut a);
                    out.push(Entry { title, icon: Some((path.clone(), 0)), path, args: buf_string(&a), dir: String::new() });
                }
            }
        }
    }
    out
}

fn take(p: PWSTR) -> String {
    unsafe {
        let s = p.to_string().unwrap_or_default();
        CoTaskMemFree(Some(p.0 as *const _));
        s
    }
}

/// Tiêu đề nhóm "Gần đây"/"Thường dùng"/"Tác vụ" theo ngôn ngữ của Windows.
fn known_name(frequent: bool) -> String {
    super::menu::tr(if frequent { "Frequent" } else { "Recent" }, if frequent { "Thường dùng" } else { "Gần đây" }).to_string()
}

/// Jump list của một app (exe chữ thường), như taskbar gốc hiện khi chuột phải.
pub fn for_app(exe: &str, hwnd: Option<isize>, lnk: Option<&str>) -> Vec<Group> {
    com();
    let mut groups: Vec<Group> = Vec::new();
    let exe_l = exe.to_lowercase();
    let id = aumid(hwnd, lnk, exe);
    // Tìm file jump list có lối tắt trỏ tới đúng chương trình này (mới nhất, bỏ qua hồ sơ tạm).
    let mut found: Option<Vec<Cat>> = None;
    if !exe_l.is_empty() {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            let dir = std::path::PathBuf::from(appdata).join(r"Microsoft\Windows\Recent\CustomDestinations");
            let mut files: Vec<(std::time::SystemTime, std::path::PathBuf)> = std::fs::read_dir(dir)
                .map(|r| r.flatten().filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path()))).collect())
                .unwrap_or_default();
            files.sort_by(|a, b| b.0.cmp(&a.0));
            let name = std::path::Path::new(&exe_l).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let needle: Vec<u8> = name.encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
            for (_, f) in files.iter().take(80) {
                let Ok(b) = std::fs::read(f) else { continue };
                // Lọc nhanh: file có nhắc tới tên chương trình (không phân biệt hoa thường thì đọc kỹ ở dưới)
                let lower: Vec<u8> = b.iter().map(|c| c.to_ascii_lowercase()).collect();
                if needle.is_empty() || !lower.windows(needle.len()).any(|w| w == needle.as_slice()) {
                    continue;
                }
                let Some(cats) = parse_custom(&b) else { continue };
                let entries = cats.iter().flat_map(|c| match c {
                    Cat::Custom(_, e) | Cat::Tasks(e) => e.iter().collect::<Vec<_>>(),
                    Cat::Known(_) => Vec::new(),
                });
                let mut matched = false;
                let mut temp = false;
                for e in entries {
                    if expand(&e.path).to_lowercase() == exe_l {
                        matched = true;
                    }
                    if e.args.to_lowercase().contains("--user-data-dir") {
                        temp = true;
                    }
                }
                if matched && !temp {
                    found = Some(cats);
                    break;
                }
            }
        }
    }
    let mut tasks = Vec::new();
    match found {
        Some(cats) => {
            for c in cats {
                match c {
                    Cat::Custom(name, e) if !e.is_empty() => groups.push(Group { name, entries: e }),
                    Cat::Known(k) => {
                        let frequent = k == -1;
                        let e = known_list(&id, frequent);
                        if !e.is_empty() {
                            groups.push(Group { name: known_name(frequent), entries: e });
                        }
                    }
                    Cat::Tasks(e) if !e.is_empty() => tasks = e,
                    _ => {}
                }
            }
        }
        None => {
            let e = known_list(&id, false);
            if !e.is_empty() {
                groups.push(Group { name: known_name(false), entries: e });
            }
        }
    }
    if !tasks.is_empty() {
        groups.push(Group { name: super::menu::tr("Tasks", "Tác vụ").to_string(), entries: tasks });
    }
    groups
}

/// Mở một mục của jump list.
pub fn run(e: &Entry) {
    let (p, a, d) = (wide(&e.path), wide(&e.args), wide(&e.dir));
    unsafe {
        ShellExecuteW(
            HWND::default(),
            PCWSTR::null(),
            PCWSTR(p.as_ptr()),
            if e.args.is_empty() { PCWSTR::null() } else { PCWSTR(a.as_ptr()) },
            if e.dir.is_empty() { PCWSTR::null() } else { PCWSTR(d.as_ptr()) },
            windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        );
    }
}
