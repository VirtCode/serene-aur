use std::collections::HashMap;
use std::io::BufRead;
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc};
use anyhow::Context;
use tokio::fs;
use tokio::sync::{Mutex, RwLock};
use crate::package::Package;

const FILE: &str = "serene.json";

pub type PackageStoreRef = Arc<RwLock<PackageStore>>;

pub struct PackageStore {
    packages: HashMap<String, Package>
}

impl PackageStore {

    pub async fn init() -> anyhow::Result<Self> {
        let mut s = PackageStore {
            packages: HashMap::new()
        };

        s.load().await?;
        Ok(s)
    }

    async fn load(&mut self) -> anyhow::Result<()>{
        if !Path::new(FILE).is_file() { return Ok(()) }

        let string = fs::read_to_string(FILE).await
            .context("failed to read serene database from file")?;

        let vec: Vec<Package> = serde_json::from_str(&string)
            .context("failed to deserialize serene database")?;

        self.packages.clear();
        for x in vec {
            self.packages.insert(x.base.clone(), x);
        }

        Ok(())
    }

    async fn save(&self) -> anyhow::Result<()> {
        let string = serde_json::to_string(&self.packages.values().collect::<Vec<&Package>>())
            .context("failed to serialize serene database")?;

        fs::write(FILE, string).await
            .context("failed to write serene database to file")?;

        Ok(())
    }
    
    pub fn peek(&self) -> Vec<&Package> {
        self.packages.values().collect()
    }

    pub fn has(&self, base: &str) -> bool {
        self.packages.contains_key(base)
    }

    pub fn get(&self, base: &str) -> Option<Package> {
        self.packages.get(base).cloned()
    }

    pub async fn update(&mut self, package: Package) -> anyhow::Result<()> {
        self.packages.insert(package.base.clone(), package);
        self.save().await
    }
}