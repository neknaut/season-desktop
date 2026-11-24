//! Đọc bài đang phát (Spotify, YouTube trên trình duyệt, Media Player...) qua
//! Windows Media Session, chạy ở luồng riêng để không làm giật taskbar.

use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use windows::Media::Control::{GlobalSystemMediaTransportControlsSessionManager as Manager, GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status};

#[derive(Clone, Default, PartialEq)]
pub struct MediaInfo {
    pub has: bool,
    pub playing: bool,
    pub title: String,
    pub artist: String,
}

pub struct Media {
    pub info: Arc<Mutex<MediaInfo>>,
    tx: Sender<()>,
}

impl Media {
    pub fn start() -> Self {
        let info = Arc::new(Mutex::new(MediaInfo::default()));
        let (tx, rx) = channel::<()>();
        let shared = info.clone();
        std::thread::spawn(move || {
            unsafe {
                let _ = windows::Win32::System::WinRT::RoInitialize(windows::Win32::System::WinRT::RO_INIT_MULTITHREADED);
            }
            let Ok(mgr) = Manager::RequestAsync().and_then(|op| op.get()) else { return };
            loop {
                let toggle = match rx.recv_timeout(Duration::from_millis(1500)) {
                    Ok(()) => true,
                    Err(RecvTimeoutError::Timeout) => false,
                    Err(RecvTimeoutError::Disconnected) => break,
                };
                let read = || -> windows::core::Result<MediaInfo> {
                    let s = mgr.GetCurrentSession()?;
                    if toggle {
                        let _ = s.TryTogglePlayPauseAsync()?.get();
                    }
                    let p = s.TryGetMediaPropertiesAsync()?.get()?;
                    let playing = s.GetPlaybackInfo()?.PlaybackStatus()? == Status::Playing;
                    Ok(MediaInfo { has: true, playing, title: p.Title()?.to_string_lossy(), artist: p.Artist()?.to_string_lossy() })
                };
                let m = read().unwrap_or_default();
                if let Ok(mut g) = shared.lock() {
                    *g = m;
                }
            }
        });
        Self { info, tx }
    }

    pub fn toggle(&self) {
        let _ = self.tx.send(());
    }

    pub fn get(&self) -> MediaInfo {
        self.info.lock().map(|g| g.clone()).unwrap_or_default()
    }
}
