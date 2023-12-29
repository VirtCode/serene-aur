use std::error::Error;
use chrono::{DateTime, Local, Utc};
use colored::Colorize;
use cron_descriptor::cronparser::cron_expression_descriptor::get_description_cron;
use serde::{Deserialize, Serialize};
use crate::config::Config;
use crate::web::post;

#[derive(Serialize, Deserialize)]
struct PackageInfo {
    base: String,
    added: DateTime<Utc>,
    enabled: bool,
    clean: bool,
    devel: bool,
    schedule: String,
}

impl PackageInfo {
    fn print(&self) {
        println!("{} {}", self.base.bold(),
            if self.devel { "(devel)" } else { "" }.dimmed()
        );

        println!("added: {}",
                 self.added.with_timezone(&Local).format("%c")
        );

        println!("status: {} {}",
            if self.enabled { "enabled".green() } else { "disabled".red() },
            if self.clean { "clean" } else { "" }
        );

        println!("schedule: {} (UTC)",
                 get_description_cron(&self.schedule)
                     .unwrap_or_else(|_| "could not parse cron".to_owned())
        )
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum PackageAddRequest {
    Aur { name: String },
    Custom { url: String, devel: bool }
}

pub fn add_aur(c: &Config, name: &str) {
    info!("Adding package {} from the AUR...", name.italic());

    match post::<PackageAddRequest, PackageInfo>(c, "package/add", PackageAddRequest::Aur { name: name.to_owned() }) {
        Ok(info) => {
            info!("Successfully added package {}", info.base);
            info.print();
        }
        Err(e) => { e.print() }
    }
}