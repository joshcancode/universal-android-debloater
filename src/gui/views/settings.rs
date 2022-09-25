use crate::core::config::{Config, DeviceSettings, GeneralSettings, BackupSettings};
use crate::core::save::{backup_phone, list_available_backup_user, restore_backup};
use crate::core::save::{list_available_backups, BACKUP_DIR};
use crate::core::sync::{User, Phone};
use crate::core::theme::Theme;
use crate::core::utils::{open_url, string_to_theme};
use crate::gui::perform_adb_commands;
use crate::gui::style;
use crate::gui::widgets::package_row::PackageRow;

use iced::widget::{button, checkbox, column, container, pick_list, radio, row, text, Space};
use iced::{Alignment, Command, Element, Length, Renderer};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Settings {
    pub general: GeneralSettings,
    pub device: DeviceSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            general: Config::load_configuration_file().general,
            device: DeviceSettings::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    LoadDeviceSettings,
    ExpertMode(bool),
    DisableMode(bool),
    MultiUserMode(bool),
    ApplyTheme(Theme),
    UrlPressed(PathBuf),
    BackupSelected(String),
    BackupDevice,
    RestoreDevice,
    DeviceBackedUp(Result<(), String>),
    BackupUserSelected(User),
    Nothing,
}

impl Settings {
    pub fn update(
        &mut self,
        phone: &Phone,
        packages: &Vec<Vec<PackageRow>>,
        msg: Message,
    ) -> Command<Message> {
        match msg {
            Message::ExpertMode(toggled) => {
                self.general.expert_mode = toggled;
                debug!("Config change: {:?}", self);
                Config::save_changes(self, &phone.adb_id);
                Command::none()
            }
            Message::DisableMode(toggled) => {
                if phone.android_sdk >= 23 {
                    self.device.disable_mode = toggled;
                    debug!("Config change: {:?}", self);
                    Config::save_changes(self, &phone.adb_id);
                }
                Command::none()
            }
            Message::MultiUserMode(toggled) => {
                self.device.multi_user_mode = toggled;
                debug!("Config change: {:?}", self);
                Config::save_changes(self, &phone.adb_id);
                Command::none()
            }
            Message::ApplyTheme(theme) => {
                self.general.theme = theme.to_string();
                debug!("Config change: {:?}", self);
                Config::save_changes(self, &phone.adb_id);
                Command::none()
            }
            Message::UrlPressed(url) => {
                open_url(url);
                Command::none()
            }
            Message::LoadDeviceSettings => {
                let backups = list_available_backups(&*BACKUP_DIR.join(phone.adb_id.clone()));
                match Config::load_configuration_file()
                    .devices
                    .iter()
                    .find(|d| d.device_id == phone.adb_id)
                {
                    Some(device) => {
                        self.device = device.clone();
                        self.device.backup = BackupSettings {
                            backups: backups.clone(),
                            selected: backups.first().cloned(),
                            users: phone.user_list.clone(),
                            selected_user: phone.user_list.first().copied()
                        };
                    }
                    None => {
                        self.device = DeviceSettings {
                            device_id: phone.adb_id.clone(),
                            multi_user_mode: phone.android_sdk > 21,
                            disable_mode: false,
                            backup: BackupSettings {
                                backups: backups.clone(),
                                selected: backups.first().cloned(),
                                users: phone.user_list.clone(),
                                selected_user: phone.user_list.first().copied(),
                            }
                        }
                    }
                };
                Command::none()
            }
            Message::BackupSelected(path) => {
                self.device.backup.selected = Some(path.clone());
                list_available_backup_user(path);
                Command::none()
            }
            Message::BackupDevice => Command::perform(
                backup_phone(
                    phone.user_list.clone(),
                    self.device.device_id.clone(),
                    packages.clone(),
                ),
                Message::DeviceBackedUp,
            ),
            Message::DeviceBackedUp(_) => {
                self.device.backup.backups =
                    list_available_backups(&*BACKUP_DIR.join(phone.adb_id.clone()));
                self.device.backup.selected = self.device.backup.backups.first().cloned();
                Command::none()
            }
            Message::BackupUserSelected(user) => {
                self.device.backup.selected_user = Some(user);
                Command::none()
            }
            Message::RestoreDevice => {
                let actions = restore_backup(
                    self.device.backup.selected.as_ref().unwrap().to_string(),
                    self.device.backup.selected_user
                ).unwrap();

                let mut commands = vec![];
                for action in actions {
                    commands.push(Command::perform(
                        perform_adb_commands(action, 0, "Restore".to_string()),
                        |_| Message::Nothing
                        )
                    );
                }
                Command::batch(commands)
            }
            Message::Nothing => {
                Command::none()
            }
        }
    }

    pub fn view(&self, phone: &Phone) -> Element<Message, Renderer<Theme>> {
        let radio_btn_theme = Theme::ALL
            .iter()
            .fold(row![].spacing(10), |column, option| {
                column.push(
                    radio(
                        format!("{}", option.clone()),
                        *option,
                        Some(string_to_theme(self.general.theme.clone())),
                        Message::ApplyTheme,
                    )
                    .size(23),
                )
            });
        let theme_ctn = container(radio_btn_theme)
            .padding(10)
            .width(Length::Fill)
            .height(Length::Shrink)
            .style(style::Container::Frame);

        let expert_mode_checkbox = checkbox(
            "Allow to uninstall packages marked as \"unsafe\" (I KNOW WHAT I AM DOING)",
            self.general.expert_mode,
            Message::ExpertMode,
        )
        .style(style::CheckBox::SettingsEnabled);

        let expert_mode_descr =
            text("Most of unsafe packages are known to bootloop the device if removed.")
                .style(style::Text::Commentary)
                .size(15);

        let warning_ctn = container(
            row![
                text("The following settings only affect the currently selected device :")
                    .style(style::Text::Danger),
                text(phone.model.to_owned())
            ]
            .spacing(7),
        )
        .padding(10)
        .width(Length::Fill)
        .style(style::Container::BorderedFrame);

        let multi_user_mode_descr =
            text("Disabling this setting will typically prevent affecting your work profile")
                .style(style::Text::Commentary)
                .size(15);

        let multi_user_mode_checkbox = checkbox(
            "Affect all the users of the phone (not only the selected user)",
            self.device.multi_user_mode,
            Message::MultiUserMode,
        )
        .style(style::CheckBox::SettingsEnabled);

        let disable_checkbox_style = if phone.android_sdk >= 23 {
            style::CheckBox::SettingsEnabled
        } else {
            style::CheckBox::SettingsDisabled
        };

        let disable_mode_descr =
            text("In some cases, it can be better to disable a package instead of uninstalling it")
                .style(style::Text::Commentary)
                .size(15);

        let unavailable_btn = button(text("Unavailable").size(13))
            .on_press(Message::UrlPressed(PathBuf::from(
                "https://github.com/0x192/universal-android-debloater/wiki/FAQ#\
                    why-is-the-disable-mode-setting-not-available-for-my-device",
            )))
            .height(Length::Units(22))
            .style(style::Button::Unavailable);

        // Disabling package without root isn't really possible before Android Oreo (8.0)
        // see https://github.com/0x192/universal-android-debloater/wiki/ADB-reference
        let disable_mode_checkbox = checkbox(
            "Clear and disable packages instead of uninstalling them",
            self.device.disable_mode,
            Message::DisableMode,
        )
        .style(disable_checkbox_style);

        let disable_setting_row = if phone.android_sdk >= 23 {
            row![
                disable_mode_checkbox,
                Space::new(Length::Fill, Length::Shrink),
            ]
            .width(Length::Fill)
        } else {
            row![
                disable_mode_checkbox,
                Space::new(Length::Fill, Length::Shrink),
                unavailable_btn,
            ]
            .width(Length::Fill)
        };

        let general_ctn = container(column![expert_mode_checkbox, expert_mode_descr].spacing(10))
            .padding(10)
            .width(Length::Fill)
            .height(Length::Shrink)
            .style(style::Container::Frame);

        let device_specific_ctn = container(
            column![
                multi_user_mode_checkbox,
                multi_user_mode_descr,
                disable_setting_row,
                disable_mode_descr,
            ]
            .spacing(10),
        )
        .padding(10)
        .width(Length::Fill)
        .height(Length::Shrink)
        .style(style::Container::Frame);

        let backup_pick_list = pick_list(
            self.device.backup.backups.clone(),
            self.device.backup.selected.clone(),
            Message::BackupSelected,
        );

        let backup_user_pick_list = pick_list(
            self.device.backup.users.clone(),
            self.device.backup.selected_user.clone(),
            Message::BackupUserSelected,
        );

        let backup_btn = button("Backup")
            .padding(5)
            .on_press(Message::BackupDevice)
            .style(style::Button::Primary);

        let restore_btn = button("Restore")
            .padding(5)
            .on_press(Message::RestoreDevice)
            .style(style::Button::Primary);

        let backup_ctn = container(
            row![
                if self.device.backup.backups.is_empty() {
                    row![
                        text("No backup").style(style::Text::Commentary),
                        restore_btn,
                        "Restore the state of the phone",
                    ]
                    .spacing(10)
                    .align_items(Alignment::Center)
                } else {
                    row![
                        backup_pick_list,
                        restore_btn,
                        backup_user_pick_list,
                        "Restore the state of the phone",
                    ]
                    .spacing(10)
                    .align_items(Alignment::Center)
                },
                Space::new(Length::Fill, Length::Shrink),
                backup_btn,
            ]
            .spacing(10)
            .align_items(Alignment::Center),
        )
        .padding(10)
        .width(Length::Fill)
        .height(Length::Shrink)
        .style(style::Container::Frame);

        let content = column![
            text("Theme").size(25),
            theme_ctn,
            text("General").size(25),
            general_ctn,
            text("Current device").size(25),
            warning_ctn,
            device_specific_ctn,
            backup_ctn,
        ]
        .width(Length::Fill)
        .spacing(20);

        container(content)
            .padding(10)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
