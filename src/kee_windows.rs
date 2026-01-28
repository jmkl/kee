#![allow(unused)]
use std::ffi::OsString;
use std::fmt::Write;
use std::{
    os::windows::ffi::OsStringExt,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Accessibility::{
    HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent, WINEVENTPROC,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EVENT_SYSTEM_FOREGROUND, GetMessageW, MSG, WINEVENT_OUTOFCONTEXT,
};

use crate::kee_manager::{CALLBACK_CHANNEL, KeeEvent};

// ============================================================================
// Type Definitions
// ============================================================================

pub type Hwnd = *mut std::ffi::c_void;
type Handle = *mut std::ffi::c_void;
type Dword = u32;
type Bool = i32;
type LParam = isize;
type HMonitor = isize;
type Hdc = isize;

// ============================================================================
// Constants
// ============================================================================
// SetWindowPos constants
const HWND_TOP: Hwnd = 0 as Hwnd;
const HWND_BOTTOM: Hwnd = 1 as Hwnd;
const HWND_TOPMOST: Hwnd = (-1isize) as Hwnd;
const HWND_NOTOPMOST: Hwnd = (-2isize) as Hwnd;

// SetWindowPos flags
const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_NOREDRAW: u32 = 0x0008;
const SWP_NOACTIVATE: u32 = 0x0010;
const SWP_FRAMECHANGED: u32 = 0x0020;
const SWP_SHOWWINDOW: u32 = 0x0040;
const SWP_HIDEWINDOW: u32 = 0x0080;
const SWP_NOCOPYBITS: u32 = 0x0100;
const SWP_NOOWNERZORDER: u32 = 0x0200;
const SWP_NOSENDCHANGING: u32 = 0x0400;

const SW_RESTORE: i32 = 9;
const SW_SHOW: i32 = 5;
const SW_MINIMIZE: i32 = 6;
const SW_MAXIMIZE: i32 = 3;
const TRUE: Bool = 1;
const FALSE: Bool = 0;
const PROCESS_QUERY_INFORMATION: Dword = 0x0400;
const PROCESS_VM_READ: Dword = 0x0010;
const SM_CXSCREEN: i32 = 0;
const SM_CYSCREEN: i32 = 1;
const MONITOR_DEFAULTTONEAREST: Dword = 2;
const MONITOR_DEFAULTTOPRIMARY: Dword = 1;
const MONITOR_DEFAULTTONULL: Dword = 0;
const MONITORINFOF_PRIMARY: Dword = 1;

pub static SUPPRESS_MODS: AtomicBool = AtomicBool::new(false);

// ============================================================================
// Monitor Structures
// ============================================================================

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MonitorInfo {
    pub cb_size: u32,
    pub rc_monitor: RECT,
    pub rc_work: RECT,
    pub dw_flags: Dword,
}

impl Default for MonitorInfo {
    fn default() -> Self {
        Self {
            cb_size: std::mem::size_of::<MonitorInfo>() as u32,
            rc_monitor: RECT::default(),
            rc_work: RECT::default(),
            dw_flags: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MonitorDetails {
    pub handle: HMonitor,
    pub width: i32,
    pub height: i32,
    pub x: i32,
    pub y: i32,
    pub work_area: RECT,
    pub is_primary: bool,
}

type MonitorEnumProc = unsafe extern "system" fn(
    hmonitor: HMonitor,
    hdc: Hdc,
    lp_rc_monitor: *mut RECT,
    dw_data: LParam,
) -> Bool;

#[link(name = "user32")]
unsafe extern "system" {
    fn EnumWindows(
        lpEnumFunc: unsafe extern "system" fn(Hwnd, LParam) -> Bool,
        lParam: LParam,
    ) -> Bool;
    fn IsWindowVisible(hWnd: Hwnd) -> Bool;
    fn IsIconic(hWnd: Hwnd) -> Bool;
    fn GetForegroundWindow() -> HWND;
    fn IsWindow(hWnd: Hwnd) -> Bool;
    fn GetWindowTextLengthW(hWnd: Hwnd) -> i32;

    fn GetClassNameW(hwnd: Hwnd, lpclassname: *mut u16, nmaxcount: i32) -> i32;
    fn GetWindowTextW(hWnd: Hwnd, lpString: *mut u16, nMaxCount: i32) -> i32;
    fn GetWindowRect(hwnd: Hwnd, rect: *mut RECT) -> Bool;
    fn SetForegroundWindow(hWnd: Hwnd) -> Bool;
    fn BringWindowToTop(hWnd: Hwnd) -> Bool;
    fn ShowWindow(hWnd: Hwnd, nCmdShow: i32) -> Bool;
    fn GetWindowThreadProcessId(hWnd: Hwnd, lpdwProcessId: *mut Dword) -> Dword;
    fn SetFocus(hWnd: Hwnd) -> Hwnd;
    fn SetActiveWindow(hWnd: Hwnd) -> Hwnd;
    fn AttachThreadInput(idAttach: Dword, idAttachTo: Dword, fAttach: Bool) -> Bool;
    fn GetSystemMetrics(n_index: i32) -> i32;
    fn GetClientRect(hwnd: Hwnd, lp_rect: *mut RECT) -> Bool;

    // Monitor functions
    fn MonitorFromWindow(hwnd: Hwnd, dw_flags: Dword) -> HMonitor;
    fn MonitorFromPoint(pt: Point, dw_flags: Dword) -> HMonitor;
    fn GetMonitorInfoW(hmonitor: HMonitor, lpmi: *mut MonitorInfo) -> Bool;
    fn EnumDisplayMonitors(
        hdc: Hdc,
        lprc_clip: *const RECT,
        lpfn_enum: Option<MonitorEnumProc>,
        dw_data: LParam,
    ) -> Bool;
    // Window positioning
    fn SetWindowPos(
        hWnd: Hwnd,
        hWndInsertAfter: Hwnd,
        X: i32,
        Y: i32,
        cx: i32,
        cy: i32,
        uFlags: u32,
    ) -> Bool;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn OpenProcess(dwDesiredAccess: Dword, bInheritHandle: Bool, dwProcessId: Dword) -> Handle;
    fn CloseHandle(hObject: Handle) -> Bool;
    fn QueryFullProcessImageNameW(
        hProcess: Handle,
        dwFlags: Dword,
        lpExeName: *mut u16,
        lpdwSize: *mut Dword,
    ) -> Bool;
}

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub struct SafeHWND(pub Hwnd);

unsafe impl Send for SafeHWND {}
unsafe impl Sync for SafeHWND {}

impl SafeHWND {
    pub fn new(hwnd: Hwnd) -> Self {
        SafeHWND(hwnd)
    }

    pub fn as_hwnd(&self) -> Hwnd {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WinSize {
    pub width: i32,
    pub height: i32,
}

impl std::fmt::Display for WinSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {})", self.width, self.height)
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct WinPos {
    pub x: i32,
    pub y: i32,
}
impl std::fmt::Display for WinPos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowInfo {
    pub hwnd: SafeHWND,
    title: String,
    exe_path: String,
    class_name: String,
    size: WinSize,
    position: WinPos,
    workspace: i32,
}

impl WindowInfo {
    pub fn name(&self) -> String {
        Path::new(&self.exe_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("UNKNOWN")
            .to_string()
    }

    pub fn class(&self) -> &String {
        &self.class_name
    }
    pub fn workspace(&self) -> i32 {
        self.workspace
    }

    pub fn size(&self) -> &WinSize {
        &self.size
    }

    pub fn position(&self) -> &WinPos {
        &self.position
    }

    pub fn title(&self) -> String {
        self.title.replace(" ", "-").to_lowercase().to_string()
    }

    pub fn exe_path(&self) -> &String {
        &self.exe_path
    }

    /// Bring this window to the front
    pub fn bring_to_front(&self) -> Result<(), String> {
        WindowManager::bring_to_front(self.hwnd.as_hwnd())
    }

    /// Check if this window is minimized
    pub fn is_minimized(&self) -> bool {
        WindowManager::is_minimized(self.hwnd.as_hwnd())
    }

    /// Check if this window is visible
    pub fn is_visible(&self) -> bool {
        WindowManager::is_visible(self.hwnd.as_hwnd())
    }

    /// Restore this window if minimized
    pub fn restore(&self) -> Result<(), String> {
        WindowManager::restore_window(self.hwnd.as_hwnd())
    }

    /// Maximize this window
    pub fn maximize(&self) -> Result<(), String> {
        WindowManager::maximize_window(self.hwnd.as_hwnd())
    }

    /// Minimize this window
    pub fn minimize(&self) -> Result<(), String> {
        WindowManager::minimize_window(self.hwnd.as_hwnd())
    }

    /// Get the monitor this window is on
    pub fn get_monitor(&self) -> Option<MonitorDetails> {
        MonitorManager::get_monitor_from_window(self.hwnd.as_hwnd())
    }
    /// Move this window to specific position
    pub fn move_to(&self, x: i32, y: i32) -> Result<(), String> {
        WindowManager::move_window(self.hwnd.as_hwnd(), x, y)
    }

    /// Resize this window
    pub fn resize(&self, width: i32, height: i32) -> Result<(), String> {
        WindowManager::resize_window(self.hwnd.as_hwnd(), width, height)
    }

    /// Set both position and size
    pub fn set_rect(&self, x: i32, y: i32, width: i32, height: i32) -> Result<(), String> {
        WindowManager::set_window_rect(self.hwnd.as_hwnd(), x, y, width, height)
    }

    /// Center window on current monitor
    pub fn center_on_monitor(&self) -> Result<(), String> {
        if let Some(monitor) = self.get_monitor() {
            let new_x = monitor.x + (monitor.width - self.size.width) / 2;
            let new_y = monitor.y + (monitor.height - self.size.height) / 2;
            self.move_to(new_x, new_y)
        } else {
            Err("Could not get monitor information".to_string())
        }
    }

    /// Fit window to monitor work area
    pub fn fit_to_monitor(&self) -> Result<(), String> {
        if let Some(monitor) = self.get_monitor() {
            let work_area = monitor.work_area;
            let width = work_area.right - work_area.left;
            let height = work_area.bottom - work_area.top;
            self.set_rect(work_area.left, work_area.top, width, height)
        } else {
            Err("Could not get monitor information".to_string())
        }
    }
}

// ============================================================================
// Window Manager
// ============================================================================

pub struct WindowManager;

impl WindowManager {
    /// Bring a window to the front (most reliable method)
    pub fn bring_to_front(hwnd: Hwnd) -> Result<(), String> {
        unsafe {
            // Check if window is valid
            if IsWindow(hwnd) == FALSE {
                return Err("Invalid window handle".to_string());
            }

            // If minimized, restore it first
            if IsIconic(hwnd) == TRUE {
                ShowWindow(hwnd, SW_RESTORE);
            }

            // Method 1: Try SetForegroundWindow (most common)
            if SetForegroundWindow(hwnd) == TRUE {
                return Ok(());
            }

            // Method 2: If Method 1 fails, use the workaround
            Self::force_window_to_front(hwnd)
        }
    }
    /// Move window to specific position
    pub fn move_window(hwnd: Hwnd, x: i32, y: i32) -> Result<(), String> {
        unsafe {
            if IsWindow(hwnd) == FALSE {
                return Err("Invalid window handle".to_string());
            }

            if SetWindowPos(hwnd, HWND_TOP, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER) != FALSE {
                Ok(())
            } else {
                Err("Failed to move window".to_string())
            }
        }
    }

    /// Resize window to specific size
    pub fn resize_window(hwnd: Hwnd, width: i32, height: i32) -> Result<(), String> {
        unsafe {
            if IsWindow(hwnd) == FALSE {
                return Err("Invalid window handle".to_string());
            }

            if SetWindowPos(
                hwnd,
                HWND_TOP,
                0,
                0,
                width,
                height,
                SWP_NOMOVE | SWP_NOZORDER,
            ) != FALSE
            {
                Ok(())
            } else {
                Err("Failed to resize window".to_string())
            }
        }
    }

    /// Move and resize window in one call
    pub fn set_window_rect(
        hwnd: Hwnd,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<(), String> {
        unsafe {
            if IsWindow(hwnd) == FALSE {
                return Err("Invalid window handle".to_string());
            }

            if SetWindowPos(hwnd, HWND_TOP, x, y, width, height, SWP_NOZORDER) != FALSE {
                Ok(())
            } else {
                Err("Failed to set window rect".to_string())
            }
        }
    }

    /// Make window always on top
    pub fn set_always_on_top(hwnd: Hwnd, always_on_top: bool) -> Result<(), String> {
        unsafe {
            if IsWindow(hwnd) == FALSE {
                return Err("Invalid window handle".to_string());
            }

            let hwnd_after = if always_on_top {
                HWND_TOPMOST
            } else {
                HWND_NOTOPMOST
            };

            if SetWindowPos(hwnd, hwnd_after, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE) != FALSE {
                Ok(())
            } else {
                Err("Failed to set always on top".to_string())
            }
        }
    }

    /// Force window to front using thread attachment technique
    fn force_window_to_front(hwnd: Hwnd) -> Result<(), String> {
        unsafe {
            // Get the foreground window
            let foreground = GetForegroundWindow();
            let foreground_hwnd = foreground.0 as Hwnd;

            // Get thread IDs
            let foreground_thread = GetWindowThreadProcessId(foreground_hwnd, std::ptr::null_mut());
            let target_thread = GetWindowThreadProcessId(hwnd, std::ptr::null_mut());
            let current_thread = GetCurrentThreadId();

            // Attach to the foreground thread to gain permission
            if foreground_thread != current_thread {
                AttachThreadInput(current_thread, foreground_thread, TRUE);
            }
            if target_thread != current_thread && target_thread != foreground_thread {
                AttachThreadInput(current_thread, target_thread, TRUE);
            }

            // Bring window to front
            BringWindowToTop(hwnd);
            ShowWindow(hwnd, SW_SHOW);
            SetForegroundWindow(hwnd);
            SetFocus(hwnd);
            SetActiveWindow(hwnd);

            // Detach threads
            if foreground_thread != current_thread {
                AttachThreadInput(current_thread, foreground_thread, FALSE);
            }
            if target_thread != current_thread && target_thread != foreground_thread {
                AttachThreadInput(current_thread, target_thread, FALSE);
            }

            Ok(())
        }
    }

    /// Check if window is minimized
    pub fn is_minimized(hwnd: Hwnd) -> bool {
        unsafe { IsIconic(hwnd) == TRUE }
    }

    /// Check if window is visible
    pub fn is_visible(hwnd: Hwnd) -> bool {
        unsafe { IsWindowVisible(hwnd) == TRUE }
    }

    /// Restore a minimized window
    pub fn restore_window(hwnd: Hwnd) -> Result<(), String> {
        unsafe {
            if IsWindow(hwnd) == FALSE {
                return Err("Invalid window handle".to_string());
            }

            ShowWindow(hwnd, SW_RESTORE);
            Ok(())
        }
    }

    /// Maximize window
    pub fn maximize_window(hwnd: Hwnd) -> Result<(), String> {
        unsafe {
            if IsWindow(hwnd) == FALSE {
                return Err("Invalid window handle".to_string());
            }

            ShowWindow(hwnd, SW_MAXIMIZE);
            Ok(())
        }
    }

    /// Minimize window
    pub fn minimize_window(hwnd: Hwnd) -> Result<(), String> {
        unsafe {
            if IsWindow(hwnd) == FALSE {
                return Err("Invalid window handle".to_string());
            }

            ShowWindow(hwnd, SW_MINIMIZE);
            Ok(())
        }
    }
}

// ============================================================================
// Monitor Manager
// ============================================================================

pub struct MonitorManager;

impl MonitorManager {
    /// Get primary monitor size
    pub fn get_primary_monitor_size() -> (i32, i32) {
        unsafe {
            let width = GetSystemMetrics(SM_CXSCREEN);
            let height = GetSystemMetrics(SM_CYSCREEN);
            (width, height)
        }
    }

    /// Get monitor from window
    pub fn get_monitor_from_window(hwnd: Hwnd) -> Option<MonitorDetails> {
        unsafe {
            let hmonitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            if hmonitor == 0 {
                return None;
            }
            Self::get_monitor_details(hmonitor)
        }
    }

    /// Get monitor from point
    pub fn get_monitor_from_point(x: i32, y: i32) -> Option<MonitorDetails> {
        unsafe {
            let point = Point { x, y };
            let hmonitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
            if hmonitor == 0 {
                return None;
            }
            Self::get_monitor_details(hmonitor)
        }
    }

    /// Get monitor info
    fn get_monitor_info(hmonitor: HMonitor) -> Option<MonitorInfo> {
        unsafe {
            let mut monitor_info = MonitorInfo::default();
            if GetMonitorInfoW(hmonitor, &mut monitor_info) != FALSE {
                Some(monitor_info)
            } else {
                None
            }
        }
    }

    /// Get monitor details
    pub fn get_monitor_details(hmonitor: HMonitor) -> Option<MonitorDetails> {
        Self::get_monitor_info(hmonitor).map(|info| {
            let rect = info.rc_monitor;
            MonitorDetails {
                handle: hmonitor,
                width: rect.right - rect.left,
                height: rect.bottom - rect.top,
                x: rect.left,
                y: rect.top,
                work_area: info.rc_work,
                is_primary: (info.dw_flags & MONITORINFOF_PRIMARY) != 0,
            }
        })
    }

    /// Get monitor size
    pub fn get_monitor_size(hmonitor: HMonitor) -> Option<(i32, i32)> {
        Self::get_monitor_details(hmonitor).map(|details| (details.width, details.height))
    }

    /// Get current monitor size (from foreground window)
    pub fn get_current_monitor_size() -> Option<(i32, i32)> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_invalid() {
                return None;
            }
            let hmonitor = MonitorFromWindow(hwnd.0 as Hwnd, MONITOR_DEFAULTTONEAREST);
            if hmonitor == 0 {
                return None;
            }
            Self::get_monitor_size(hmonitor)
        }
    }

    /// Enumerate all monitors
    pub fn enumerate_monitors() -> Vec<MonitorDetails> {
        unsafe {
            let mut monitors = Vec::new();

            EnumDisplayMonitors(
                0,
                std::ptr::null(),
                Some(enum_monitor_callback),
                &mut monitors as *mut Vec<MonitorDetails> as LParam,
            );

            monitors
        }
    }

    /// Get primary monitor
    pub fn get_primary_monitor() -> Option<MonitorDetails> {
        Self::enumerate_monitors()
            .into_iter()
            .find(|m| m.is_primary)
    }

    /// Get monitor count
    pub fn get_monitor_count() -> usize {
        Self::enumerate_monitors().len()
    }
}

// ============================================================================
// Internal Helper Functions
// ============================================================================

unsafe extern "system" fn enum_windows_callback(hwnd: Hwnd, lparam: LParam) -> Bool {
    let windows = unsafe { &mut *(lparam as *mut Vec<WindowInfo>) };

    // Skip invisible windows
    if unsafe { IsWindowVisible(hwnd) } == FALSE {
        return TRUE;
    }

    let class_name = get_class_name(hwnd).unwrap_or_else(|| String::from("UNKNOWN_CLASS"));

    // Get window title
    let title = match get_window_title(hwnd) {
        Some(t) if !t.is_empty() => t,
        _ => return TRUE, // Skip windows without titles
    };
    let (size, position) = get_window_size_and_position(hwnd);
    // Get process executable path
    let exe_path = get_process_path(hwnd).unwrap_or_else(|| String::from("UNKNOWN_EXE_PATH"));
    let (y, h) = (position.y, size.height);
    windows.push(WindowInfo {
        hwnd: SafeHWND::new(hwnd),
        title,
        exe_path,
        class_name,
        size,
        position,
        workspace: get_current_workspace(y, h),
    });

    TRUE // Continue enumeration
}

unsafe extern "system" fn win_event_proc(
    _h_win_event_hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _id_event_thread: u32,
    _dwms_event_time: u32,
) {
    let hwnd = hwnd.0;
    let class_name = get_class_name(hwnd).unwrap_or_else(|| String::from("UNKNOWN_CLASS"));

    // Get window title
    let title = match get_window_title(hwnd) {
        Some(t) if !t.is_empty() => t,
        _ => return, // Skip windows without titles
    };
    let (size, position) = get_window_size_and_position(hwnd);
    // Get process executable path
    let exe_path = get_process_path(hwnd).unwrap_or_else(|| String::from("UNKNOWN_EXE_PATH"));
    let (y, h) = (position.y, size.height);
    let win_info = WindowInfo {
        hwnd: SafeHWND::new(hwnd),
        title,
        exe_path,
        class_name,
        size,
        position,
        workspace: get_current_workspace(y, h),
    };
    if let Some(tx) = CALLBACK_CHANNEL.get() {
        let _ = tx.try_send(KeeEvent::OnWindowChange(win_info));
    }
}

unsafe extern "system" fn enum_monitor_callback(
    hmonitor: HMonitor,
    _hdc: Hdc,
    _lp_rc_monitor: *mut RECT,
    dw_data: LParam,
) -> Bool {
    let monitors = unsafe { &mut *(dw_data as *mut Vec<MonitorDetails>) };

    if let Some(details) = MonitorManager::get_monitor_details(hmonitor) {
        monitors.push(details);
    }

    TRUE // Continue enumeration
}

fn get_class_name(hwnd: Hwnd) -> Option<String> {
    let mut buffer: [u16; 256] = [0; 256];
    let copied = unsafe { GetClassNameW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };

    if copied > 0 {
        let class_name = OsString::from_wide(&buffer[..copied as usize]);
        Some(class_name.to_string_lossy().into_owned())
    } else {
        None
    }
}

fn get_window_title(hwnd: Hwnd) -> Option<String> {
    unsafe {
        let length = GetWindowTextLengthW(hwnd);
        if length == 0 {
            return None;
        }

        let mut buffer: Vec<u16> = vec![0; (length + 1) as usize];
        let copied = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);

        if copied > 0 {
            buffer.truncate(copied as usize);
            Some(OsString::from_wide(&buffer).to_string_lossy().into_owned())
        } else {
            None
        }
    }
}

fn get_process_path(hwnd: Hwnd) -> Option<String> {
    unsafe {
        // Get process ID
        let mut process_id: Dword = 0;
        GetWindowThreadProcessId(hwnd, &mut process_id);

        if process_id == 0 {
            return None;
        }

        // Open process handle
        let process_handle = OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
            FALSE,
            process_id,
        );

        if process_handle.is_null() {
            return None;
        }

        // Query executable path
        let mut path_buffer: Vec<u16> = vec![0; 1024];
        let mut size: Dword = path_buffer.len() as Dword;

        let result =
            QueryFullProcessImageNameW(process_handle, 0, path_buffer.as_mut_ptr(), &mut size);

        CloseHandle(process_handle);

        if result != FALSE && size > 0 {
            path_buffer.truncate(size as usize);
            Some(
                OsString::from_wide(&path_buffer)
                    .to_string_lossy()
                    .into_owned(),
            )
        } else {
            None
        }
    }
}

fn get_window_rect(hwnd: Hwnd) -> RECT {
    unsafe {
        let mut rect = RECT::default();
        GetWindowRect(hwnd, &mut rect);
        rect
    }
}

fn get_window_size_and_position(hwnd: Hwnd) -> (WinSize, WinPos) {
    let rect = get_window_rect(hwnd);

    let x = rect.left;
    let y = rect.top;
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;

    (WinSize { width, height }, WinPos { x, y })
}
const MONITOR_H: i32 = 1440;
fn get_current_workspace(y: i32, window_h: i32) -> i32 {
    if window_h <= 0 {
        return 0;
    }

    // Shift so partially visible windows stay in workspace 0
    let shifted = y + window_h;

    if shifted <= 0 {
        (-shifted).div_euclid(MONITOR_H)
    } else {
        shifted.div_euclid(MONITOR_H)
    }
}

// ============================================================================
// Public API
// ============================================================================

pub fn get_current_active_window() -> Option<WindowInfo> {
    unsafe {
        let hwnd = GetForegroundWindow();
        let hwnd_ptr = hwnd.0 as Hwnd;

        if hwnd_ptr.is_null() || hwnd_ptr == 0 as Hwnd {
            return None;
        }

        // Check if window is valid and visible
        if IsWindow(hwnd_ptr) == FALSE || IsWindowVisible(hwnd_ptr) == FALSE {
            return None;
        }

        // Get window info
        let class_name = get_class_name(hwnd_ptr).unwrap_or_else(|| String::from("UNKNOWN_CLASS"));

        let title = match get_window_title(hwnd_ptr) {
            Some(t) if !t.is_empty() => t,
            _ => String::from(""),
        };

        let (size, position) = get_window_size_and_position(hwnd_ptr);
        let exe_path =
            get_process_path(hwnd_ptr).unwrap_or_else(|| String::from("UNKNOWN_EXE_PATH"));
        let (y, h) = (position.y, size.height);

        Some(WindowInfo {
            hwnd: SafeHWND::new(hwnd_ptr),
            title,
            exe_path,
            class_name,
            size,
            position,
            workspace: get_current_workspace(y, h),
        })
    }
}
/// List all visible windows with titles
pub fn list_windows() -> Vec<WindowInfo> {
    let mut windows: Vec<WindowInfo> = Vec::new();

    unsafe {
        EnumWindows(enum_windows_callback, &mut windows as *mut _ as LParam);
    }

    windows
}

/// Find a window by executable name (case-insensitive, partial match)
pub fn find_window_by_exe_name(exe_name: &str) -> Option<WindowInfo> {
    let windows = list_windows();
    let search = exe_name.to_lowercase();

    windows
        .into_iter()
        .find(|w| w.name().to_lowercase().contains(&search))
}

/// Find all windows by executable name (case-insensitive, partial match)
pub fn find_windows_by_exe_name(exe_name: &str) -> Vec<WindowInfo> {
    let windows = list_windows();
    let search = exe_name.to_lowercase();

    windows
        .into_iter()
        .filter(|w| w.name().to_lowercase().contains(&search))
        .collect()
}

/// Find a window by title (case-insensitive, partial match)
pub fn find_window_by_title(title: &str) -> Option<WindowInfo> {
    let windows = list_windows();
    let search = title.to_lowercase();

    windows
        .into_iter()
        .find(|w| w.title.to_lowercase().contains(&search))
}

/// Find all windows by title (case-insensitive, partial match)
pub fn find_windows_by_title(title: &str) -> Vec<WindowInfo> {
    let windows = list_windows();
    let search = title.to_lowercase();

    windows
        .into_iter()
        .filter(|w| w.title.to_lowercase().contains(&search))
        .collect()
}

pub fn spawn_active_window_listener() {
    unsafe {
        let hook = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        );

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            // _ = TranslateMessage(&msg);
            // _ = DispatchMessageW(&msg);
        }

        UnhookWinEvent(hook);
    }
}

#[cfg(test)]
mod test_kee_window {
    use crate::kee_windows::{MonitorManager, list_windows};

    #[test]
    fn test_windows() {
        for n in list_windows() {
            println!("{:?}", &n);
        }
    }

    #[test]
    fn test_monitors() {
        println!(
            "Primary monitor: {:?}",
            MonitorManager::get_primary_monitor_size()
        );
        println!(
            "Current monitor: {:?}",
            MonitorManager::get_current_monitor_size()
        );

        let monitors = MonitorManager::enumerate_monitors();
        println!("Found {} monitors:", monitors.len());
        for (i, monitor) in monitors.iter().enumerate() {
            println!("Monitor {}: {:?}", i + 1, monitor);
        }
    }

    #[test]
    fn test_window_monitor() {
        for window in list_windows() {
            if let Some(monitor) = window.get_monitor() {
                println!(
                    "{} is on monitor: {}x{} at ({}, {})",
                    window.name(),
                    monitor.width,
                    monitor.height,
                    monitor.x,
                    monitor.y
                );
            }
        }
    }
}
