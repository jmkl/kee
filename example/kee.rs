use kee::{Event, Kee};
use parking_lot::Mutex;
use std::{sync::Arc, time::Instant};

enum SearchMode {
    Title,
    Name,
}

struct CycleApps {
    apps: Vec<String>,
    active_index: usize,
}
impl CycleApps {
    pub fn new(apps: Vec<String>) -> Self {
        Self {
            apps,
            active_index: 0,
        }
    }
    pub fn next(&mut self) -> usize {
        self.active_index = (self.active_index + 1) % self.apps.len();
        self.active_index
    }
    pub fn get_app(&self, index: usize) -> Option<&String> {
        self.apps.get(index)
    }
}

fn find_window(search_mode: SearchMode, payload: &str) {
    let start = Instant::now();

    if let Some(window) = kee::list_windows().iter().find(|k| match search_mode {
        SearchMode::Title => k.title().to_uppercase() == payload,
        SearchMode::Name => k.name().to_uppercase() == payload,
    }) {
        _ = window.bring_to_front();
    }
    println!("Execute in: {}ms", start.elapsed().as_millis());
}

fn main() -> anyhow::Result<()> {
    let mut kee = Kee::new();
    let apps = Arc::new(Mutex::new(CycleApps::new(kee.get_apps())));
    let clone_apps = apps.clone();
    kee.on_message(move |event| match event {
        Event::Keys(_, f) => {
            if let Some(app) = f.split_once("::") {
                match app.0 {
                    "app" => match app.1 {
                        "CYCLE" => {
                            let mut clone_apps = clone_apps.lock();

                            let app = {
                                let index = clone_apps.next();
                                let app = clone_apps.get_app(index);
                                app
                            };
                            if let Some(app) = app {
                                if let Some(app) = app.split_once(":") {
                                    if app.0 == "T" {
                                        find_window(SearchMode::Title, app.1);
                                    }
                                } else {
                                    find_window(SearchMode::Name, app);
                                }
                            }
                        }
                        "PHOTOSHOP" => {
                            find_window(SearchMode::Name, app.1);
                        }
                        tsockee if app.1.starts_with("TSOCKEE") => {
                            if let Some(start) = f.find("(") {
                                if let Some(end) = f.rfind(")") {
                                    if end <= start {
                                        return;
                                    }
                                    let part = f[start + 1..end].trim();
                                    find_window(SearchMode::Title, part);
                                }
                            }
                        }
                        _ => {}
                    },
                    _ => {
                        //NO OP
                    }
                }
            }
        }
        _ => {}
    })
    .run();

    Ok(())
}
