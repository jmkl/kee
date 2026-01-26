#![allow(unused)]
use crate::{TKeePair, TKeePairList};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

fn dotfile_root_dir() -> anyhow::Result<PathBuf> {
    let user = std::env::var("USERPROFILE")?;
    let config = Path::new(&user).join(".config").join("kee");
    if !config.exists() {
        std::fs::create_dir_all(&config)?;
    }
    Ok(config)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct KeeConfig {
    pub kees: TKeePairList,
    pub apps: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Config {
    kee_config: KeeConfig,
}

impl Config {
    pub fn new() -> Self {
        Self {
            kee_config: Self::read_config_files(),
        }
    }
    fn read_config_files() -> KeeConfig {
        let default_pairs = TKeePairList(vec![TKeePair::new("M-1", "app::LaunchZen")]);
        let kee_config = KeeConfig {
            kees: default_pairs,
            apps: Vec::new(),
        };
        if let Ok(dir) = dotfile_root_dir() {
            let p = Path::new(&dir).join("kee.json");
            if p.exists() {
                if let Ok(s) = std::fs::read_to_string(p) {
                    if let Ok(result) = serde_json::from_str::<KeeConfig>(&s) {
                        return result;
                    }
                }
            } else {
                if let Ok(content) = serde_json::to_string_pretty(&kee_config) {
                    _ = std::fs::write(p, content);
                }
            }
        }
        //fallback
        kee_config
    }
    pub fn get_config(&self) -> KeeConfig {
        self.kee_config.clone()
    }
}

#[cfg(test)]
mod test_config {
    use super::*;
    #[test]
    fn test_config() -> anyhow::Result<()> {
        let config = Config::new();
        println!("{:?}", config.get_config());
        Ok(())
    }
}
