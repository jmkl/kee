#![allow(unused)]
use std::ffi::OsString;
use std::{
    os::windows::ffi::OsStringExt,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
use windows::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{
        HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos,
    },
};

// ============================================================================
// Type Definitions
// ============================================================================

pub type Hwnd = *mut std::ffi::c_void;
type Handle = *mut std::ffi::c_void;
type Dword = u32;
type Bool = i32;
type LParam = isize;

// ============================================================================
// Constants
// ============================================================================

const SW_RESTORE: i32 = 9;
const SW_SHOW: i32 = 5;
const SW_MINIMIZE: i32 = 6;
const SW_MAXIMIZE: i32 = 3;
const TRUE: Bool = 1;
const FALSE: Bool = 0;
const PROCESS_QUERY_INFORMATION: Dword = 0x0400;
const PROCESS_VM_READ: Dword = 0x0010;

pub static SUPPRESS_MODS: AtomicBool = AtomicBool::new(false);

// ============================================================================
// Windows API Function Declarations
// ============================================================================

#[link(name = "user32")]
unsafe extern "system" {
    fn EnumWindows(
        lpEnumFunc: unsafe extern "system" fn(Hwnd, LParam) -> Bool,
        lParam: LParam,
    ) -> Bool;
    fn IsWindowVisible(hWnd: Hwnd) -> Bool;
    fn IsIconic(hWnd: Hwnd) -> Bool;
    fn IsWindow(hWnd: Hwnd) -> Bool;
    fn GetWindowTextLengthW(hWnd: Hwnd) -> i32;
    fn GetClassNameW(hwnd: Hwnd, lpclassname: *mut u16, nmaxcount: i32) -> i32;
    fn GetWindowTextW(hWnd: Hwnd, lpString: *mut u16, nMaxCount: i32) -> i32;
    fn SetForegroundWindow(hWnd: Hwnd) -> Bool;
    fn BringWindowToTop(hWnd: Hwnd) -> Bool;
    fn ShowWindow(hWnd: Hwnd, nCmdShow: i32) -> Bool;
    fn GetWindowThreadProcessId(hWnd: Hwnd, lpdwProcessId: *mut Dword) -> Dword;
    fn SetFocus(hWnd: Hwnd) -> Hwnd;
    fn SetActiveWindow(hWnd: Hwnd) -> Hwnd;
    fn AttachThreadInput(idAttach: Dword, idAttachTo: Dword, fAttach: Bool) -> Bool;
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

#[derive(Debug, Clone, Copy)]
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

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub hwnd: SafeHWND,
    title: String,
    exe_path: String,
    class_name: String,
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

    // Get process executable path
    let exe_path = get_process_path(hwnd).unwrap_or_else(|| String::from("UNKNOWN_EXE_PATH"));

    windows.push(WindowInfo {
        hwnd: SafeHWND::new(hwnd),
        title,
        exe_path,
        class_name,
    });

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

// ============================================================================
// Example Usage
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_windows() {
        let windows = list_windows();
        println!("Found {} windows", windows.len());

        for (i, window) in windows.iter().take(5).enumerate() {
            println!("{}. {} - {}", i + 1, window.name(), window.title);
        }
    }

    #[test]
    fn test_find_and_focus() {
        // Find a window by exe name
        if let Some(window) = find_window_by_exe_name("notepad") {
            println!("Found Notepad: {}", window.title);

            if window.is_minimized() {
                println!("Window is minimized, restoring...");
                window.restore().ok();
            }

            println!("Bringing to front...");
            window.bring_to_front().ok();
        } else {
            println!("Notepad not found");
        }
    }
}

// Example main function
#[cfg(example)]
fn main() {
    println!("=== Window Manager Demo ===\n");

    // List all windows
    let windows = list_windows();
    println!("Found {} windows:\n", windows.len());

    for (i, window) in windows.iter().take(10).enumerate() {
        println!("{}. {} ({})", i + 1, window.name(), window.title);
    }

    // Find and focus a specific window
    println!("\n=== Finding Chrome ===");
    if let Some(chrome) = find_window_by_exe_name("chrome") {
        println!("Found: {} - {}", chrome.name(), chrome.title);
        println!("Is minimized: {}", chrome.is_minimized());
        println!("Is visible: {}", chrome.is_visible());

        println!("\nBringing Chrome to front...");
        match chrome.bring_to_front() {
            Ok(_) => println!("Success!"),
            Err(e) => println!("Error: {}", e),
        }
    } else {
        println!("Chrome not found");
    }

    // Find all instances of an app
    println!("\n=== Finding all Visual Studio Code windows ===");
    let vscode_windows = find_windows_by_exe_name("code");
    println!("Found {} VS Code windows", vscode_windows.len());

    for window in vscode_windows {
        println!("  - {}", window.title);
    }
}
