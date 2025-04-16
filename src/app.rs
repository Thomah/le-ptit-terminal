use crate::eventbrite_attendees::Attendee;
use crate::eventbrite_auth::{load_config, save_config};
use arboard::Clipboard;
use crossterm::event::KeyCode;
use log::{debug, error};

pub enum AppView {
    MainMenu,
    ListNextEventAttendeesMenu,
    SettingsMenu,
    SetClientIdPopup,
    SetClientSecretPopup,
}

pub struct App {
    pub view: AppView,
    pub main_menu_index: usize,
    pub settings_menu_index: usize,
    pub attendees: Vec<Attendee>,
    pub input_buffer: String,
    pub event_date: Option<String>,
    pub selected_row: usize,
    pub selected_col: usize,
}

impl App {
    pub fn new() -> Self {
        Self {
            view: AppView::MainMenu,
            main_menu_index: 0,
            settings_menu_index: 0,
            attendees: vec![],
            input_buffer: String::new(),
            event_date: Some(String::new()),
            selected_row: 0,
            selected_col: 0,
        }
    }

    pub fn load_attendees(&mut self, token: &str) {
        debug!("Attempting to load attendees with token: {}", token);
        match crate::eventbrite_attendees::get_attendees_from_api(token) {
            Ok((attendees, event_date)) => {
                debug!("Successfully fetched {} attendees", attendees.len());
                self.attendees = attendees;
                self.event_date = Some(event_date);
            }
            Err(err) => {
                error!("Failed to fetch attendees: {}", err);
            }
        }
    }

    pub fn handle_input(&mut self, key: KeyCode) -> Option<String> {
        debug!("Handling input: {:?}", key);
        match self.view {
            AppView::MainMenu => match key {
                KeyCode::Esc => {
                    debug!("Quit selected in MainMenu");
                    return Some("quit".to_string());
                }
                KeyCode::Down => {
                    self.main_menu_index = (self.main_menu_index + 1) % 2;
                    debug!("MainMenu index changed to {}", self.main_menu_index);
                }
                KeyCode::Up => {
                    self.main_menu_index = if self.main_menu_index == 0 {
                        1
                    } else {
                        self.main_menu_index - 1
                    };
                    debug!("MainMenu index changed to {}", self.main_menu_index);
                }
                KeyCode::Enter => match self.main_menu_index {
                    0 => {
                        debug!("Navigating to ListNextEventAttendeesMenu");
                        self.view = AppView::ListNextEventAttendeesMenu;
                        match crate::eventbrite_auth::get_access_token() {
                            Ok(token) => {
                                debug!("Access token retrieved: {}", token);
                                self.load_attendees(&token);
                            }
                            Err(err) => {
                                error!("Failed to retrieve access token: {}", err);
                            }
                        }
                    }
                    1 => {
                        debug!("Navigating to SettingsMenu");
                        self.view = AppView::SettingsMenu;
                    }
                    _ => {}
                },
                _ => {
                    debug!("Unhandled key in MainMenu: {:?}", key);
                }
            },
            AppView::ListNextEventAttendeesMenu => match key {
                KeyCode::Esc => {
                    self.view = AppView::MainMenu;
                }
                KeyCode::Up => {
                    if self.selected_row > 0 {
                        self.selected_row -= 1;
                    }
                }
                KeyCode::Down => {
                    if self.selected_row < self.attendees.len() - 1 {
                        self.selected_row += 1;
                    }
                }
                KeyCode::Left => {
                    if self.selected_col > 0 {
                        self.selected_col -= 1;
                    }
                }
                KeyCode::Right => {
                    if self.selected_col < 6 {
                        self.selected_col += 1;
                    }
                }
                KeyCode::Char('c') => {
                    let value = self.get_selected_cell_value();
                    debug!("Copied value: {}", value);
                    if let Err(err) = Clipboard::new().and_then(|mut clipboard| clipboard.set_text(value)) {
                        error!("Failed to copy to clipboard: {}", err);
                    } else {
                        debug!("Value successfully copied to clipboard");
                    }
                }
                _ => {}
            },
            AppView::SettingsMenu => match key {
                KeyCode::Esc => {
                    self.view = AppView::MainMenu;
                }
                KeyCode::Down => {
                    self.settings_menu_index = (self.settings_menu_index + 1) % 2;
                    debug!("SettingsMenu index changed to {}", self.settings_menu_index);
                }
                KeyCode::Up => {
                    self.settings_menu_index = if self.settings_menu_index == 0 {
                        2
                    } else {
                        self.settings_menu_index - 1
                    };
                    debug!("SettingsMenu index changed to {}", self.settings_menu_index);
                }
                KeyCode::Enter => match self.settings_menu_index {
                    0 => {
                        debug!("Navigating to SetClientIdPopup");
                        self.view = AppView::SetClientIdPopup;
                        self.input_buffer.clear();
                    }
                    1 => {
                        debug!("Navigating to SetClientSecretPopup");
                        self.view = AppView::SetClientSecretPopup;
                        self.input_buffer.clear();
                    }
                    2 => {
                        debug!("Returning to MainMenu from SettingsMenu");
                        self.view = AppView::MainMenu;
                    }
                    _ => {
                        debug!("Unhandled SettingsMenu index: {}", self.settings_menu_index);
                    }
                },
                _ => {
                    debug!("Unhandled key in SettingsMenu: {:?}", key);
                }
            },
            AppView::SetClientIdPopup => match key {
                KeyCode::Esc => {
                    debug!("Exiting SetClientIdPopup, returning to SettingsMenu");
                    self.view = AppView::SettingsMenu;
                }
                KeyCode::Enter => {
                    debug!("Saving CLIENT_ID: {}", self.input_buffer.trim());
                    let mut config = load_config().unwrap_or_default();
                    config.client_id = Some(self.input_buffer.trim().to_string());
                    save_config(&config).expect("Failed to save configuration");
                    self.view = AppView::SettingsMenu;
                }
                KeyCode::Char(c) => {
                    debug!("Appending '{}' to input buffer in SetClientIdPopup", c);
                    self.input_buffer.push(c);
                }
                KeyCode::Backspace => {
                    debug!("Removing last character from input buffer in SetClientIdPopup");
                    self.input_buffer.pop();
                }
                _ => {
                    debug!("Unhandled key in SetClientIdPopup: {:?}", key);
                }
            },
            AppView::SetClientSecretPopup => match key {
                KeyCode::Esc => {
                    debug!("Exiting SetClientSecretPopup, returning to SettingsMenu");
                    self.view = AppView::SettingsMenu;
                }
                KeyCode::Enter => {
                    debug!("Saving CLIENT_SECRET: {}", self.input_buffer.trim());
                    let mut config = load_config().unwrap_or_default();
                    config.client_secret = Some(self.input_buffer.trim().to_string());
                    save_config(&config).expect("Failed to save configuration");
                    self.view = AppView::SettingsMenu;
                }
                KeyCode::Char(c) => {
                    debug!("Appending '{}' to input buffer in SetClientSecretPopup", c);
                    self.input_buffer.push(c);
                }
                KeyCode::Backspace => {
                    debug!("Removing last character from input buffer in SetClientSecretPopup");
                    self.input_buffer.pop();
                }
                _ => {
                    debug!("Unhandled key in SetClientSecretPopup: {:?}", key);
                }
            },
        }
        None
    }

    fn get_selected_cell_value(&self) -> String {
        if self.selected_row >= self.attendees.len() {
            return String::new();
        }

        let attendee = &self.attendees[self.selected_row];
        match self.selected_col {
            0 => attendee.profile.first_name.clone().unwrap_or_default(),
            1 => attendee.profile.last_name.clone().unwrap_or_default(),
            2 => attendee.profile.email.clone().unwrap_or_default(),
            3 => attendee.profile.cell_phone.clone().unwrap_or_default(),
            4 => attendee.ticket_class_name.clone().unwrap_or_default(),
            5 => attendee.created.clone(),
            _ => String::new(),
        }
    }
}