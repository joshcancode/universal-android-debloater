use crate::core::config::DeviceSettings;
use crate::core::sync::{action_handler, Action, CorePackage, Phone, User};
use crate::core::utils::DisplayablePath;
use crate::gui::widgets::package_row::PackageRow;
use crate::CACHE_DIR;
use serde::{Deserialize, Serialize};
use static_init::dynamic;
use std::fs;
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
            user_backup.packages.push(CorePackage {
                name: p.name.clone(),
                state: p.state,
            })
        }
        backup.users.push(user_backup);
    }

    match serde_json::to_string_pretty(&backup) {
        Ok(json) => {
            let backup_path = &*BACKUP_DIR.join(device_id);

            if let Err(e) = fs::create_dir_all(backup_path) {
                error!("BACKUP: could not create backup dir: {}", e);
                return Err(e.to_string());
            };

            let backup_filename = format!("{}.json", chrono::Local::now().format("%Y-%m-%d-%H-%M"));

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

pub fn list_available_backups(dir: &Path) -> Vec<DisplayablePath> {
    match fs::read_dir(dir) {
        Ok(files) => files
            .filter_map(|e| e.ok())
            .map(|e| DisplayablePath { path: e.path() })
            .collect::<Vec<_>>(),
        Err(_) => vec![],
    }
}

pub fn list_available_backup_user(backup: DisplayablePath) -> Vec<User> {
    match fs::read_to_string(backup.path) {
        Ok(data) => {
            let phone_backup: PhoneBackup =
                serde_json::from_str(&data).expect("Unable to parse backup file");

            let mut users = vec![];
            for u in phone_backup.users {
                users.push(User { id: u.id, index: 0 });
            }
            users
        }
        Err(e) => {
            error!("[BACKUP]: Selected backup file not found: {}", e);
            vec![]
        }
    }
}


// TODO: we need to change the way package state change are handled
// Better to try to match the wanted state instead of applying the "reverse" ADB command
pub fn restore_backup(
    selected_device: &Phone,
    settings: &DeviceSettings,
) -> Result<Vec<String>, String> {
    match fs::read_to_string(settings.backup.selected.as_ref().unwrap().path.clone()) {
        Ok(data) => {
            let phone_backup: PhoneBackup =
                serde_json::from_str(&data).expect("Unable to parse backup file");

            let mut commands = vec![];
            for u in phone_backup.users {
                for packages in u.packages {
                    commands.extend(change_pkg_state_commands(
                        &settings.backup.selected_user.unwrap(),
                        &packages,
                        selected_device,
                        settings,
                        &Action::RestoreDevice,
                    ));
                }
            }
            Ok(commands)
        }
        Err(e) => Err("[BACKUP]: ".to_owned() + &e.to_string()),
    }
}

pub fn apply_pkg_state_commands(
    selected_user: &User,
    backup_pkg: &Option<CorePackage>,
    phone_pkg: &CorePackage
    phone: &Phone,
    settings: &DeviceSettings,
    action: &Action,
) -> Vec<String> {

    if phone_pkg.state == backup_pkg.state {
        return vec![];
    }

    let commands = match backup_pkg.state {
        PackageState::Enabled => {
            let commands = match phone_pkg.state {
                PackageState::Uninstalled => vec!["pm disable-user", "am force-stop", "pm clear"],
                PackageState::Disabled => vec!["pm uninstall"],
                _ => vec![]
            };

            match phone.android_sdk {
                sdk if sdk >= 23 => commands,            // > Android Marshmallow (6.0)
                21 | 22 => vec!["pm hide", "pm clear"],  // Android Lollipop (5.x)
                19 | 20 => vec!["pm block", "pm clear"], // Android KitKat (4.4/4.4W)
                _ => vec!["pm uninstall"], // Disable mode is unavailable on older devices because the specific ADB commands need root
            }
        }
        PackageState::Uninstalled => {
            match phone.android_sdk {
                i if i >= 23 => vec!["cmd package install-existing"],
                21 | 22 => vec!["pm unhide"],
                19 | 20 => vec!["pm unblock", "pm clear"],
                _ => vec![], // Impossible action already prevented by the GUI
            }
        }
        // `pm enable` doesn't work without root before Android 6.x and this is most likely the same on even older devices too.
        // Should never happen as disable_mode is unavailable on older devices
        PackageState::Disabled => match phone.android_sdk {
            i if i >= 23 => vec!["pm enable"],
            _ => vec!["pm enable"],
        },
        PackageState::All => vec![], // This can't happen (like... never)
    };

    if phone.android_sdk < 21 {
        request_builder(commands, &package.name, &[])
    } else {
        match action {
            Action::Misc => {
                if settings.multi_user_mode {
                    request_builder(commands, &package.name, &phone.user_list)
                } else {
                    request_builder(commands, &package.name, &[*selected_user])
                }
            }
            Action::RestoreDevice => request_builder(commands, &package.name, &phone.user_list),
        }
    }
}