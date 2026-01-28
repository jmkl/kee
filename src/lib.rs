mod beep;
mod kee_keys;
mod kee_manager;
mod kee_windows;
use flume::{Receiver, Sender, unbounded};
pub use kee_manager::TsckKeeManager;
use parking_lot::{Mutex, RwLock};
use std::sync::Arc;
mod macros;
use crate::{beep::BeepController, kee_manager::Modifier};
pub use kee_keys::{TKeePair, TKeePairList};
pub use kee_windows::list_windows;
pub use kee_windows::{SafeHWND, WindowInfo, get_current_active_window};
type EventHandler = Arc<dyn Fn(&Event) + Send + Sync + 'static>;

#[derive(Debug, Clone)]
pub enum Event {
    Keys(String, String),
    WindowChange(WindowInfo),
    Shutdown,
}

pub struct Kee {
    hotkey_manager: TsckKeeManager,
    sender: Sender<Event>,
    receiver: Receiver<Event>,
    handler: Option<EventHandler>,
    beep_controller: Option<Arc<Mutex<BeepController>>>,
    current_keypairs: Arc<RwLock<Vec<TKeePair>>>,
}

impl Kee {
    pub fn new() -> Self {
        let (tx, rx) = unbounded();
        Self {
            hotkey_manager: TsckKeeManager::new(),
            sender: tx,
            receiver: rx,
            handler: None,
            beep_controller: BeepController::new().ok().map(|f| Arc::new(Mutex::new(f))),
            current_keypairs: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn on_message<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn(&Event) + Send + Sync + 'static,
    {
        self.handler = Some(Arc::new(f));
        self
    }

    fn register_hotkeys(&self, kees: Vec<TKeePair>) -> anyhow::Result<()> {
        {
            let mut current = self.current_keypairs.write();
            *current = kees;
        }

        let keypairs_ref = self.current_keypairs.clone();
        let beep_controller = self.beep_controller.clone();
        let keys = self.current_keypairs.read();
        let keys = keys.iter().map(|kp| kp.key.as_str()).collect();
        let sender = self.sender.clone();
        self.hotkey_manager
            .register_hotkeys(keys, move |cb| match cb {
                kee_manager::KeeEvent::OnKey(k) => {
                    // Read from the stored keypairs (which can be updated!)
                    let keypairs = keypairs_ref.read();
                    if let Some(pair) = keypairs.iter().find(|p| p.key == k) {
                        _ = sender.send(Event::Keys(pair.key.clone(), pair.func.clone()));
                    }
                }
                kee_manager::KeeEvent::OnModifier(modifier, state) => {
                    if modifier == Modifier::Win {
                        if let Some(controller) = beep_controller.as_ref() {
                            let mut guard = controller.lock();
                            if state {
                                guard.start();
                            } else {
                                guard.stop();
                            }
                        }
                    }
                }
                kee_manager::KeeEvent::OnWindowChange(safe_window_info) => {
                    _ = sender.send(Event::WindowChange(safe_window_info));
                }
            })?;

        Ok(())
    }

    /// Update hotkeys at runtime
    pub fn update_hotkeys(&self, kees: Vec<TKeePair>) -> anyhow::Result<()> {
        println!("Updating hotkeys with {} new bindings", kees.len());

        {
            let mut current = self.current_keypairs.write();
            *current = kees;
        }

        let keypairs_ref = self.current_keypairs.clone();
        let beep_controller = self.beep_controller.clone();
        let keys = self.current_keypairs.read();
        let keys = keys.iter().map(|kp| kp.key.as_str()).collect();
        let sender = self.sender.clone();
        self.hotkey_manager
            .update_hotkeys(keys, move |cb| match cb {
                kee_manager::KeeEvent::OnKey(k) => {
                    // Read from the stored keypairs (which were just updated!)
                    let keypairs = keypairs_ref.read();
                    if let Some(pair) = keypairs.iter().find(|p| p.key == k) {
                        _ = sender.send(Event::Keys(pair.key.clone(), pair.func.clone()));
                    }
                }
                kee_manager::KeeEvent::OnModifier(modifier, state) => {
                    if modifier == Modifier::Win {
                        if let Some(controller) = beep_controller.as_ref() {
                            let mut guard = controller.lock();
                            if state {
                                guard.start();
                            } else {
                                guard.stop();
                            }
                        }
                    }
                }
                kee_manager::KeeEvent::OnWindowChange(safe_window_info) => {
                    _ = sender.send(Event::WindowChange(safe_window_info));
                }
            })?;

        println!("Hotkeys updated successfully");
        Ok(())
    }

    pub fn run(&self, kees: Vec<TKeePair>) {
        if let Err(_) = self.register_hotkeys(kees) {
            panic!("Failed to registering hotkey");
        }

        let receiver = self.receiver.clone();
        let handler = self.handler.clone();

        std::thread::spawn(move || {
            if let Some(handler) = handler {
                while let Ok(event) = receiver.recv() {
                    if matches!(event, Event::Shutdown) {
                        break;
                    }
                    handler(&event);
                }
            }
        });
    }

    pub fn run_blocking(&self, kees: Vec<TKeePair>) {
        if let Err(_) = self.register_hotkeys(kees) {
            panic!("Failed to registering hotkey");
        }
        if let Some(ref handler) = self.handler {
            while let Ok(event) = self.receiver.recv() {
                if matches!(event, Event::Shutdown) {
                    break;
                }
                handler(&event);
            }
        }
    }

    pub fn sender(&self) -> Sender<Event> {
        self.sender.clone()
    }
}

impl Default for Kee {
    fn default() -> Self {
        Self::new()
    }
}
