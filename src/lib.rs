mod beep;
mod config;
mod kee_keys;
mod kee_manager;
mod kee_windows;
mod lexer;
use flume::{Receiver, Sender, unbounded};
pub use kee_manager::TsckKeeManager;
use parking_lot::Mutex;
use std::sync::Arc;
mod macros;
use crate::{beep::BeepController, config::Config, kee_manager::Modifier};
pub use kee_keys::{TKeePair, TKeePairList};
pub use kee_windows::{get_current_active_window, list_windows};
pub use lexer::{Command, Expr, parse_command};
type EventHandler = Arc<dyn Fn(&Event) + Send + Sync + 'static>;

#[derive(Debug, Clone)]
pub enum Event {
    Keys(String, String),
    Shutdown,
}

pub struct Kee {
    hotkey_manager: TsckKeeManager,
    sender: Sender<Event>,
    receiver: Receiver<Event>,
    handler: Option<EventHandler>,
    beep_controller: Option<Arc<Mutex<BeepController>>>,
    config: Config,
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
            config: Config::new(),
        }
    }

    pub fn on_message<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn(&Event) + Send + Sync + 'static,
    {
        self.handler = Some(Arc::new(f));
        self
    }
    pub fn get_apps(&self) -> Vec<String> {
        self.config.get_config().apps
    }

    fn register_hotkeys(&self) -> anyhow::Result<()> {
        let config = self.config.get_config();
        let keypairs = Arc::new(config.kees.0);
        let keys = keypairs.iter().map(|kp| kp.key.as_str()).collect();
        let sender = self.sender.clone();
        let cloned_pairs = keypairs.clone();
        let beep_controller = self.beep_controller.clone();
        _ = self
            .hotkey_manager
            .register_hotkeys(keys, move |cb| match cb {
                kee_manager::KeeEvent::OnKey(k) => {
                    if let Some(pair) = cloned_pairs.iter().find(|p| p.key == k) {
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
            });

        Ok(())
    }
    pub fn run(&self) {
        if let Err(_) = self.register_hotkeys() {
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

    pub fn run_blocking(&self) {
        if let Err(_) = self.register_hotkeys() {
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
