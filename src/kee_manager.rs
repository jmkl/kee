use parking_lot::Mutex;
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};
use windows::Win32::{
    Foundation::{LPARAM, LRESULT, WPARAM},
    UI::WindowsAndMessaging::{
        CallNextHookEx, GetMessageW, KBDLLHOOKSTRUCT, MSG, SetWindowsHookExW, UnhookWindowsHookEx,
        WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    },
};

use crate::tsck_keys::TsckKeeBinding;

// Event type to distinguish between hotkey and modifier events
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotkeyEvent {
    OnKey(String),              // Hotkey triggered (e.g., "Ctrl+Shift+A")
    OnModifier(Modifier, bool), // Modifier state changed (modifier, is_pressed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modifier {
    Ctrl,
    Shift,
    Alt,
    Win,
}

type TsckEventCb = std::sync::Arc<dyn Fn(HotkeyEvent) + Send + Sync>;

struct HotkeyState {
    hotkey_names: HashMap<(u16, u32), String>,
    event_callbacks: Vec<TsckEventCb>,
    key_states: [bool; 256],
}

static HOTKEY_STATE: OnceLock<Mutex<HotkeyState>> = OnceLock::new();
static CALLBACK_CHANNEL: OnceLock<flume::Sender<HotkeyEvent>> = OnceLock::new();

#[derive(Debug)]
pub struct TsckKeeManager;

impl TsckKeeManager {
    pub fn new() -> Self {
        HOTKEY_STATE.get_or_init(|| {
            Mutex::new(HotkeyState {
                hotkey_names: HashMap::new(),
                event_callbacks: Vec::new(),
                key_states: [false; 256],
            })
        });

        let (tx, rx) = flume::unbounded::<HotkeyEvent>();
        CALLBACK_CHANNEL.get_or_init(|| tx);

        let _ = std::thread::Builder::new()
            .name("hotkey-callback-executor".to_string())
            .spawn(move || {
                while let Ok(event) = rx.recv() {
                    let state = HOTKEY_STATE.get().expect("HOTKEY_STATE initialized");

                    // Get event callbacks first
                    let event_callbacks = {
                        let hotkey_state = state.lock();
                        hotkey_state.event_callbacks.clone()
                    };

                    // // Handle OnKey event with old-style callback
                    // if let HotkeyEvent::OnKey(name) = &event {
                    //     let hotkey_state = state.lock();
                    //     if let Some(cb) = hotkey_state.callbacks.values().next() {
                    //         let cb_clone = cb.clone();
                    //         drop(hotkey_state);
                    //         cb_clone(name);
                    //     }
                    // }

                    // Execute all event callbacks
                    for callback in event_callbacks {
                        callback(event.clone());
                    }
                }
            });

        let _ = std::thread::Builder::new()
            .name("hotkey-hook".to_string())
            .spawn(|| unsafe {
                let hook = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), None, 0) {
                    Ok(hhook) => hhook,
                    Err(_) => {
                        return;
                    }
                };
                let mut msg = MSG::default();

                while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                    // Message loop for hook
                }
                if let Err(e) = UnhookWindowsHookEx(hook) {
                    eprintln!("Failed to unhook keyboard hook: {:?}", e);
                }
            });
        std::thread::sleep(std::time::Duration::from_millis(100));
        Self
    }

    pub fn register_hotkeys<M>(&self, hotkeys: Vec<&str>, mod_callback: M) -> anyhow::Result<()>
    where
        M: Fn(HotkeyEvent) + Send + Sync + 'static,
    {
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
                hotkeys_map
                    .hotkey_names
                    .insert((tk, flags), hotkey_str.clone());
                println!(
                    "Registered:{} (TK=0x{:02X}, mods=0x{:04X})",
                    hotkey_str, tk, flags
                );
            }
        }
        {
            let mut hotkeys = state.lock();
            hotkeys.event_callbacks.push(Arc::new(mod_callback));
            println!("Registered event callback");
        }

        Ok(())
    }

    // New API - Register event callback for both hotkeys and modifiers
    pub fn register_event_callback<F>(&self, callback: F)
    where
        F: Fn(HotkeyEvent) + Send + Sync + 'static,
    {
        let state = HOTKEY_STATE
            .get()
            .expect("HOTKEY_STATE should be initialized in new()");

        let mut hotkeys = state.lock();
        hotkeys.event_callbacks.push(std::sync::Arc::new(callback));
        println!("Registered event callback");
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
        let hotkey_state = state.lock();

        match msg {
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                let mut key_states = hotkey_state.key_states;
                let was_pressed = key_states[vk_code as usize];
                key_states[vk_code as usize] = true;

                // Check if this is a modifier key press
                let modifier_event = match vk_code {
                    0xA2 | 0xA3 => Some(Modifier::Ctrl),  // Left/Right Ctrl
                    0xA0 | 0xA1 => Some(Modifier::Shift), // Left/Right Shift
                    0x12 => Some(Modifier::Alt),          // Alt
                    0x5B | 0x5C => Some(Modifier::Win),   // Left/Right Win
                    _ => None,
                };

                // Send modifier press event (only on first press, not repeat)
                if let Some(modifier) = modifier_event {
                    if !was_pressed {
                        if let Some(tx) = CALLBACK_CHANNEL.get() {
                            let _ = tx.try_send(HotkeyEvent::OnModifier(modifier, true));
                        }
                    }
                }

                let ctrl_pressed = key_states[0xA2 as usize] || key_states[0xA3 as usize];
                let shift_pressed = key_states[0xA0 as usize] || key_states[0xA1 as usize];
                let alt_pressed = key_states[0x12 as usize];
                let meta_pressed = key_states[0x5B as usize] || key_states[0x5C as usize];

                let mut hotkey_matched = None;

                // Find matching hotkey
                for (&(registered_vk, mod_flags), _callback) in hotkey_state.hotkey_names.iter() {
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

                            hotkey_matched = Some(matched_name);
                            break;
                        }
                    }
                }

                // Update key_states back
                drop(hotkey_state);
                let mut hotkey_state = state.lock();
                hotkey_state.key_states = key_states;
                drop(hotkey_state);

                // Send hotkey event
                if let Some(name) = hotkey_matched {
                    if let Some(tx) = CALLBACK_CHANNEL.get() {
                        let _ = tx.try_send(HotkeyEvent::OnKey(name.clone()));
                    }

                    let is_system_key = vk_code == 0x5B
                        || vk_code == 0x5C
                        || vk_code == 0x09
                        || (vk_code == 0x1B && alt_pressed);

                    !is_system_key
                } else {
                    false
                }
            }
            WM_KEYUP | WM_SYSKEYUP => {
                // Check if this is a modifier key release
                let modifier_event = match vk_code {
                    0xA2 | 0xA3 => Some(Modifier::Ctrl),
                    0xA0 | 0xA1 => Some(Modifier::Shift),
                    0x12 => Some(Modifier::Alt),
                    0x5B | 0x5C => Some(Modifier::Win),
                    _ => None,
                };

                // Send modifier release event
                if let Some(modifier) = modifier_event {
                    if let Some(tx) = CALLBACK_CHANNEL.get() {
                        let _ = tx.try_send(HotkeyEvent::OnModifier(modifier, false));
                    }
                }

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
