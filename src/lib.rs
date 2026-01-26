mod beep;
mod kee_manager;
mod tsck_keys;
mod tsck_manager;
use flume::{Receiver, Sender, unbounded};
pub use kee_manager::TsckKeeManager;
use parking_lot::Mutex;
use std::sync::Arc;
mod macros;
use crate::{beep::BeepController, kee_manager::Modifier};
pub use tsck_keys::TKeyPair;

type EventHandler = Arc<dyn Fn(&Event) + Send + Sync + 'static>;

#[derive(Debug, Clone)]
pub enum Event {
    Keys(String, String),
    Shutdown,
}

pub struct Tsck {
    hotkey_manager: TsckKeeManager,
    sender: Sender<Event>,
    receiver: Receiver<Event>,
    handler: Option<EventHandler>,
    beep_controller: Option<Arc<Mutex<BeepController>>>,
}

impl Tsck {
    pub fn new() -> Self {
        let (tx, rx) = unbounded();
        Self {
            hotkey_manager: TsckKeeManager::new(),
            sender: tx,
            receiver: rx,
            handler: None,
            beep_controller: BeepController::new().ok().map(|f| Arc::new(Mutex::new(f))),
        }
    }

    pub fn on_message<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn(&Event) + Send + Sync + 'static,
    {
        self.handler = Some(Arc::new(f));
        self
    }

    pub fn register_hotkeys(&mut self, keypairs: Vec<TKeyPair>) -> anyhow::Result<&mut Self> {
        let keypairs = Arc::new(keypairs);
        let keys = keypairs.iter().map(|kp| kp.key.as_str()).collect();
        let sender = self.sender.clone();
        let cloned_pairs = keypairs.clone();
        let beep_controller = self.beep_controller.clone();
        _ = self
            .hotkey_manager
            .register_hotkeys(keys, move |cb| match cb {
                kee_manager::HotkeyEvent::OnKey(k) => {
                    if let Some(pair) = cloned_pairs.iter().find(|p| p.key == k) {
                        _ = sender.send(Event::Keys(pair.key.clone(), pair.func.clone()));
                    }
                }
                kee_manager::HotkeyEvent::OnModifier(modifier, state) => {
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
            });

        Ok(self)
    }

    pub fn run(&self) {
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

impl Default for Tsck {
    fn default() -> Self {
        Self::new()
    }
}
