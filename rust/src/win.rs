//! Platform integration: theme detection, notifications, taskbar progress and
//! the dark window title bar.

use crate::define;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
    Unknown,
}

pub fn detect_theme() -> ThemeMode {
    match dark_light::detect() {
        Ok(dark_light::Mode::Dark) => ThemeMode::Dark,
        Ok(dark_light::Mode::Light) => ThemeMode::Light,
        _ => ThemeMode::Unknown,
    }
}

pub fn notify(title: &str, body: &str) {
    let mut n = notify_rust::Notification::new();
    n.summary(title).body(body).appname(define::APP_TITLE);
    let icon = define::base_path().join("icon-128px.png");
    if icon.is_file() {
        n.icon(&icon.to_string_lossy());
    }
    if let Err(e) = n.show() {
        log::warn!("failed to show notification: {e}");
    }
}

#[cfg(windows)]
pub use win_impl::*;

/// Opaque handle to the platform taskbar-progress object.
#[cfg(windows)]
pub type TaskbarHandle = windows::Win32::UI::Shell::ITaskbarList3;
#[cfg(not(windows))]
pub type TaskbarHandle = ();

#[cfg(windows)]
mod win_impl {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{
        ITaskbarList3, TaskbarList, TBPF_ERROR, TBPF_NOPROGRESS, TBPF_NORMAL, TBPF_PAUSED,
    };

    /// Progress bar states, mirroring `TBPFLAG`.
    #[derive(Debug, Clone, Copy)]
    pub enum ProgressState {
        None,
        Normal,
        Paused,
        Error,
    }

    pub fn com_init() {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
    }

    pub fn set_dark_titlebar(hwnd: isize, dark: bool) {
        use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};
        let value: i32 = if dark { 1 } else { 0 };
        unsafe {
            let _ = DwmSetWindowAttribute(
                HWND(hwnd as *mut _),
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &value as *const i32 as *const core::ffi::c_void,
                std::mem::size_of::<i32>() as u32,
            );
        }
    }

    pub fn create_taskbar() -> Option<ITaskbarList3> {
        unsafe {
            let tbl: ITaskbarList3 =
                CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER).ok()?;
            let _ = tbl.HrInit();
            Some(tbl)
        }
    }

    pub fn set_taskbar_progress(
        tbl: &ITaskbarList3,
        hwnd: isize,
        state: ProgressState,
        completed: u64,
        total: u64,
    ) {
        let h = HWND(hwnd as *mut _);
        let flag = match state {
            ProgressState::None => TBPF_NOPROGRESS,
            ProgressState::Normal => TBPF_NORMAL,
            ProgressState::Paused => TBPF_PAUSED,
            ProgressState::Error => TBPF_ERROR,
        };
        unsafe {
            let _ = tbl.SetProgressState(h, flag);
            if !matches!(state, ProgressState::None) {
                let _ = tbl.SetProgressValue(h, completed, total.max(1));
            }
        }
    }
}

#[cfg(not(windows))]
pub mod win_impl {
    #[derive(Debug, Clone, Copy)]
    pub enum ProgressState {
        None,
        Normal,
        Paused,
        Error,
    }
    pub fn com_init() {}
    pub fn set_dark_titlebar(_hwnd: isize, _dark: bool) {}
    pub fn create_taskbar() -> Option<()> {
        None
    }
    pub fn set_taskbar_progress(_t: &(), _hwnd: isize, _s: ProgressState, _c: u64, _t2: u64) {}
}