use crate::core::sync::{action_handler, User, Action, CorePackage};
use crate::core::uad_lists::PackageState;
use crate::core::utils::update_selection_count;
use crate::gui::views::list::Selection;
use crate::gui::widgets::package_row::PackageRow;
use crate::CACHE_DIR;
use serde::{Deserialize, Serialize};
use static_init::dynamic;
use std::fs;
use std::io::{self, prelude::*, BufReader};
use std::path::{Path, PathBuf};

#[dynamic]
pub static BACKUP_DIR: PathBuf = CACHE_DIR.join("backups");

#[derive(Default, Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
struct PhoneBackup {
    device_id: String,
    users: Vec<UserBackup>,
}

#[derive(Default, Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
struct UserBackup {
    id: u16,
    packages: Vec<CorePackage>,
}

// Backup all `Uninstalled` and `Disabled` packages
pub async fn backup_phone(
    users: Vec<User>,
    device_id: String,
    phone_packages: Vec<Vec<PackageRow>>,
) -> Result<(), String> {
    let mut backup = PhoneBackup {
        device_id: device_id.clone(),
        ..PhoneBackup::default()
    };

    for u in users {
        let mut user_backup = UserBackup {
            id: u.id,
            ..UserBackup::default()
        };

        for p in phone_packages[u.index].clone() {
            user_backup.packages.push(
                CorePackage {
                    name: p.name.clone(),
                    state: p.state
                }
            )
        }
        backup.users.push(user_backup);
    }

    match serde_json::to_string_pretty(&backup) {
        Ok(json) => {
            let backup_path = &*BACKUP_DIR.join(device_id.clone());

            if let Err(e) = fs::create_dir_all(backup_path) {
                error!("BACKUP: could not create backup dir: {}", e);
                return Err(e.to_string());
            };

            let backup_filename = format!("{}.json", chrono::Local::now().format("%Y-%m-%d-%H"));

            match fs::write(backup_path.join(backup_filename), json) {
                Ok(_) => Ok(()),
                Err(err) => Err(err.to_string()),
            }
        }
        Err(err) => {
            error!("[BACKUP]: {}", err);
            Err(err.to_string())
        }
    }
}

pub fn list_available_backups(dir: &Path) -> Vec<String> {
    match fs::read_dir(dir) {
        Ok(files) => files
            .filter_map(|e| e.ok())
            .map(|e| {
                e.path()
                    .file_stem()
                    .unwrap()
                    .to_os_string()
                    .into_string()
                    .unwrap()
            })
            .collect::<Vec<_>>(),
        Err(_) => vec![],
    }
}

pub fn list_available_backup_user(backup: String) -> Result<Vec<u16>,()> {
    match fs::read_to_string(backup) {
        Ok(data) => {
            let phone_backup: PhoneBackup =
                serde_json::from_str(&data).expect("Unable to parse backup file");

            let mut users = vec![];
            for u in phone_backup.users {
                users.push(u.id);
            }
            Ok(users)
        }
        Err(e) => {
            error!("[BACKUP]: Selected backup file not found: {}", e);
            Err(())
        }
    }
}

pub fn restore_backup(backup: String, selected_user: Option<User>) -> Result<Vec<String>, ()> {
    match fs::read_to_string(backup) {
        Ok(data) => {
            let phone_backup: PhoneBackup =
                serde_json::from_str(&data).expect("Unable to parse backup file");

            let commands = vec![];
            /*for u in phone_backup.users {
                for packages in u {
                    commands.push(action_handler(
                        selected_user.unwrap(),
                        package,
                        selected_device,
                        &settings.device,
                        Action::RestoreDevice
                    ));
                }
            }*/
            Ok(commands)
        }
        Err(e) => {
            error!("[BACKUP]: Backup file not found: {}", e);
            Err(())
        }
    }
}

// To be removed
pub async fn export_selection(packages: Vec<PackageRow>) -> Result<bool, String> {
    let selected = packages
        .iter()
        .filter(|p| p.selected)
        .map(|p| p.name.clone())
        .collect::<Vec<String>>()
        .join("\n");

    match fs::write("uad_exported_selection.txt", selected) {
        Ok(_) => Ok(true),
        Err(err) => Err(err.to_string()),
    }
}

// To be removed
pub fn import_selection(packages: &mut [PackageRow], selection: &mut Selection) -> io::Result<()> {
    let file = fs::File::open("uad_exported_selection.txt")?;
    let reader = BufReader::new(file);
    let imported_selection: Vec<String> = reader
        .lines()
        .map(|l| l.expect("Could not parse line"))
        .collect();

    *selection = Selection::default(); // should already be empty normally

    for (i, p) in packages.iter_mut().enumerate() {
        if imported_selection.contains(&p.name) {
            p.selected = true;
            selection.selected_packages.push(i);
            update_selection_count(selection, p.state, true);
        } else {
            p.selected = false;
        }
    }

    Ok(())
}
