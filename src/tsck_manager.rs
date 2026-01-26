use parking_lot::Mutex;
use std::{collections::HashMap, sync::OnceLock};
use windows::Win32::{
    Foundation::{LPARAM, LRESULT, WPARAM},
    UI::WindowsAndMessaging::{
        CallNextHookEx, GetMessageW, KBDLLHOOKSTRUCT, MSG, SetWindowsHookExW, UnhookWindowsHookEx,
        WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    },
};

use crate::tsck_keys::TsckKeeBinding;

type TsckKeeCb = std::sync::Arc<dyn Fn(&str) + Send + Sync>;

struct HotkeyState {
    callbacks: HashMap<(u16, u32), TsckKeeCb>,
    hotkey_names: HashMap<(u16, u32), String>,
    key_states: [bool; 256],
}

static HOTKEY_STATE: OnceLock<Mutex<HotkeyState>> = OnceLock::new();
static CALLBACK_CHANNEL: OnceLock<flume::Sender<(TsckKeeCb, String)>> = OnceLock::new();

#[derive(Debug)]
pub struct TsckKeeManager;

impl TsckKeeManager {
    pub fn new() -> Self {
        HOTKEY_STATE.get_or_init(|| {
            Mutex::new(HotkeyState {
                callbacks: HashMap::new(),
                hotkey_names: HashMap::new(),
                key_states: [false; 256],
            })
        });

        // CRITICAL FIX: Use channel for callback execution
        let (tx, rx) = flume::unbounded::<(TsckKeeCb, String)>();
        CALLBACK_CHANNEL.get_or_init(|| tx);

        let _ = std::thread::Builder::new()
            .name("hotkey-callback-executor".to_string())
            .spawn(move || {
                while let Ok((callback, name)) = rx.recv() {
                    callback(&name);
                    println!("CALLBACK");
                }
            });

        // Hook thread
        let _ = std::thread::Builder::new()
            .name("hotkey-hook".to_string())
            .spawn(|| unsafe {
                let hook = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), None, 0) {
                    Ok(hhook) => hhook,
                    Err(_) => {
                        return;
                    }
                };
                // let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), None, 0);
                let mut msg = MSG::default();

                while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                    // let _ = TranslateMessage(&msg);
                    // let _ = DispatchMessageW(&msg);
                }
                if let Err(e) = UnhookWindowsHookEx(hook) {
                    eprintln!("Failed to unhook keyboard hook: {:?}", e);
                }
            });
        std::thread::sleep(std::time::Duration::from_millis(100));
        Self
    }

    pub fn register_hotkey_str<F>(&self, hotkey_str: &str, callback: F) -> anyhow::Result<()>
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        let binding = TsckKeeBinding::parse(hotkey_str)?;
        let vk = binding.to_tk();
        let flags = binding.modifiers.to_flags();

        let state = HOTKEY_STATE
            .get()
            .expect("HOTKEY_STATE should be initialized in new()");

        {
            let mut hotkeys = state.lock();
            hotkeys
                .callbacks
                .insert((vk, flags), std::sync::Arc::new(callback));
            hotkeys
                .hotkey_names
                .insert((vk, flags), hotkey_str.to_string());
        }

        println!(
            "Registered hotkey: {} (VK=0x{:02X}, mods=0x{:04X})",
            hotkey_str, vk, flags
        );
        Ok(())
    }

    pub fn register_hotkeys<F>(&self, hotkeys: Vec<&str>, callback: F) -> anyhow::Result<()>
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        let callback = std::sync::Arc::new(callback);

        let bindings: Vec<(u16, u32, String)> = hotkeys
            .iter()
            .map(|hotkey_str| {
                let binding = TsckKeeBinding::parse(hotkey_str)?;
                let tk = binding.to_tk();
                let flags = binding.modifiers.to_flags();
                Ok((tk, flags, hotkey_str.to_string()))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        let state = HOTKEY_STATE
            .get()
            .expect("HOTKEY_STATE should be initialized in new()");

        {
            let mut hotkeys_map = state.lock();
            for (tk, flags, hotkey_str) in bindings {
                hotkeys_map.callbacks.insert((tk, flags), callback.clone());
                hotkeys_map
                    .hotkey_names
                    .insert((tk, flags), hotkey_str.clone());
                println!(
                    "Registered:{} (TK=0x{:02X}, mods=0x{:04X})",
                    hotkey_str, tk, flags
                );
            }
        }

        Ok(())
    }

    pub fn event_loop(&self) {
        println!("Hotkey manager running. Press registered hotkeys...");
        loop {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}

#[inline]
unsafe extern "system" fn keyboard_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }

    let vk_code = (unsafe { *(lparam.0 as *const KBDLLHOOKSTRUCT) }).vkCode as u16;
    let msg = wparam.0 as u32;

    let Some(state) = HOTKEY_STATE.get() else {
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    };

    let should_block = {
        // CRITICAL: Minimize time holding lock
        let hotkey_state = state.lock();

        match msg {
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                // Clone key_states to avoid holding lock during state update
                let mut key_states = hotkey_state.key_states;
                key_states[vk_code as usize] = true;

                let ctrl_pressed = key_states[0xA2 as usize] || key_states[0xA3 as usize];
                let shift_pressed = key_states[0xA0 as usize] || key_states[0xA1 as usize];
                let alt_pressed = key_states[0x12 as usize];
                let meta_pressed = key_states[0x5B as usize] || key_states[0x5C as usize];

                let mut callback_to_send = None;

                // Find matching hotkey
                for (&(registered_vk, mod_flags), callback) in hotkey_state.callbacks.iter() {
                    if registered_vk == vk_code {
                        let ctrl = (mod_flags & 0x0002) != 0;
                        let shift = (mod_flags & 0x0004) != 0;
                        let alt = (mod_flags & 0x0001) != 0;
                        let meta = (mod_flags & 0x0008) != 0;

                        if ctrl == ctrl_pressed
                            && shift == shift_pressed
                            && alt == alt_pressed
                            && meta == meta_pressed
                        {
                            let matched_name = hotkey_state
                                .hotkey_names
                                .get(&(registered_vk, mod_flags))
                                .cloned()
                                .unwrap_or_default();

                            callback_to_send = Some((callback.clone(), matched_name));
                            break;
                        }
                    }
                }

                // Update key_states back
                drop(hotkey_state);
                let mut hotkey_state = state.lock();
                hotkey_state.key_states = key_states;
                drop(hotkey_state);

                // Send callback to execution thread (non-blocking)
                if let Some((callback, name)) = callback_to_send {
                    if let Some(tx) = CALLBACK_CHANNEL.get() {
                        // try_send is non-blocking - critical for hook performance
                        let _ = tx.try_send((callback, name));
                    }

                    // CRITICAL: Never block Windows key or other system keys
                    // Blocking these causes deadlocks with tao's internal keyboard state
                    let is_system_key = vk_code == 0x5B // Left Windows key
                        || vk_code == 0x5C // Right Windows key
                        || vk_code == 0x09 // Tab (for Alt+Tab)
                        || (vk_code == 0x1B && alt_pressed); // Esc with Alt

                    !is_system_key
                } else {
                    false
                }
            }
            WM_KEYUP | WM_SYSKEYUP => {
                drop(hotkey_state);
                let mut hotkey_state = state.lock();
                hotkey_state.key_states[vk_code as usize] = false;
                drop(hotkey_state);
                false
            }
            _ => false,
        }
    };

    if should_block {
        LRESULT(1)
    } else {
        unsafe { CallNextHookEx(None, code, wparam, lparam) }
    }
}
