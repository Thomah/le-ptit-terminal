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
    FindByNameMenu,
}

pub struct NameSearchState {
    pub input_first_name: String,
    pub input_last_name: String,
    pub focus: usize,
    pub results: Option<Vec<(String, String)>>,
    pub results_scroll: usize,
}

impl Default for NameSearchState {
    fn default() -> Self {
        Self {
            input_first_name: String::new(),
            input_last_name: String::new(),
            focus: 0,
            results: None,
            results_scroll: 0,
        }
    }
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
    pub name_search_state: NameSearchState,
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
            name_search_state: NameSearchState::default(),
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
                    self.main_menu_index = (self.main_menu_index + 1) % 3;
                    debug!("MainMenu index changed to {}", self.main_menu_index);
                }
                KeyCode::Up => {
                    self.main_menu_index = if self.main_menu_index == 0 {
                        2
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
                    2 => {
                        debug!("Navigating to FindByNameMenu");
                        self.view = AppView::FindByNameMenu;
                        self.name_search_state = NameSearchState::default();
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
                    if let Err(err) =
                        Clipboard::new().and_then(|mut clipboard| clipboard.set_text(value))
                    {
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
            AppView::FindByNameMenu => match key {
                KeyCode::Esc => {
                    self.view = AppView::MainMenu;
                }
                KeyCode::Tab => {
                    self.name_search_state.focus = (self.name_search_state.focus + 1) % 3;
                }
                KeyCode::BackTab => {
                    if self.name_search_state.focus == 0 {
                        self.name_search_state.focus = 1;
                    } else {
                        self.name_search_state.focus -= 1;
                    }
                }
                KeyCode::Enter => {
                    // Only search if both fields are filled
                    if !self.name_search_state.input_first_name.trim().is_empty()
                        && !self.name_search_state.input_last_name.trim().is_empty()
                    {
                        self.name_search_state.results = Some(self.find_events_by_name(
                            &self.name_search_state.input_first_name,
                            &self.name_search_state.input_last_name,
                        ));
                        self.name_search_state.results_scroll = 0; // reset scroll
                    }
                }
                KeyCode::Char(c) => {
                    if self.name_search_state.focus == 0 {
                        self.name_search_state.input_first_name.push(c);
                    } else if self.name_search_state.focus == 1 {
                        self.name_search_state.input_last_name.push(c);
                    }
                }
                KeyCode::Backspace => {
                    if self.name_search_state.focus == 0 {
                        self.name_search_state.input_first_name.pop();
                    } else if self.name_search_state.focus == 1 {
                        self.name_search_state.input_last_name.pop();
                    }
                }
                KeyCode::Down => {
                    if self.name_search_state.focus == 2 {
                        if let Some(ref events) = self.name_search_state.results {
                            if !events.is_empty()
                                && self.name_search_state.results_scroll + 1 < events.len()
                            {
                                self.name_search_state.results_scroll += 1;
                            }
                        }
                    }
                }
                KeyCode::Up => {
                    if self.name_search_state.focus == 2 {
                        if self.name_search_state.results_scroll > 0 {
                            self.name_search_state.results_scroll -= 1;
                        }
                    }
                }
                _ => {}
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

    pub fn find_events_by_name(&self, first_name: &str, last_name: &str) -> Vec<(String, String)> {
        use reqwest::blocking::Client;

        let mut found_events = Vec::new();
        let token = match crate::eventbrite_auth::get_access_token() {
            Ok(token) => token,
            Err(_) => return found_events,
        };
        let client = Client::new();

        // Fetch organization ID
        let org_id = match crate::eventbrite_attendees::get_organization_id(&client, &token) {
            Some(id) => id,
            None => return found_events,
        };

        // Fetch all events (not just next)
        let res = client
            .get(&format!(
                "https://www.eventbriteapi.com/v3/organizations/{}/events/",
                org_id
            ))
            .bearer_auth(&token)
            .query(&[("order_by", "start_desc"), ("status", "completed,live")])
            .send();

        let events: Vec<crate::eventbrite_attendees::Event> = match res {
            Ok(resp) => {
                if !resp.status().is_success() {
                    return found_events;
                }
                match resp.json::<crate::eventbrite_attendees::EventsResponse>() {
                    Ok(data) => data.events,
                    Err(_) => return found_events,
                }
            }
            Err(_) => return found_events,
        };

        for event in events {
            let attendees = crate::eventbrite_attendees::get_attendees(&client, &token, &event.id);
            for attendee in attendees {
                let matches_first = attendee
                    .profile
                    .first_name
                    .as_ref()
                    .map(|n| n.eq_ignore_ascii_case(first_name.trim()))
                    .unwrap_or(false);
                let matches_last = attendee
                    .profile
                    .last_name
                    .as_ref()
                    .map(|n| n.eq_ignore_ascii_case(last_name.trim()))
                    .unwrap_or(false);
                if matches_first && matches_last {
                    found_events.push((event.name.text.clone(), event.start.local.clone()));
                    break;
                }
            }
        }
        found_events
    }
}
