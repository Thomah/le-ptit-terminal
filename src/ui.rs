use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, List, ListItem, ListState, Paragraph, Row, Table},
};

use crate::app::{App, AppView};

pub fn draw_ui(f: &mut Frame, app: &App) {
    match app.view {
        AppView::MainMenu => draw_main_menu(f, app),
        AppView::ListNextEventAttendeesMenu => draw_submenu(f, app),
        AppView::SettingsMenu => draw_settings_menu(f, app),
        AppView::SetClientIdPopup => draw_popup(f, "Enter CLIENT_ID", &app.input_buffer),
        AppView::SetClientSecretPopup => draw_popup(f, "Enter CLIENT_SECRET", &app.input_buffer),
        AppView::FindByNameMenu => draw_find_by_name_menu(f, app),
    }
}

fn draw_main_menu(f: &mut Frame, app: &App) {
    let items = vec![
        ListItem::new("Liste des participants à la prochaine maraude"),
        ListItem::new("Paramétrage"),
        ListItem::new("Rechercher un participant par nom"),
    ];

    let mut state = ListState::default();
    state.select(Some(app.main_menu_index));

    let list = List::new(items)
        .block(Block::default().title("Menu Principal").borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    // Divide the screen into two parts: the list and the status bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Min(0), // The list occupies the remaining space
                Constraint::Length(1), // Height of the status bar
            ]
            .as_ref(),
        )
        .split(f.size());

    // Render the list in the top chunk
    f.render_stateful_widget(list, chunks[0], &mut state);

    // Create and render the status bar in the bottom chunk
    let status_bar = Paragraph::new("Appuyez sur 'Échap' pour quitter")
        .style(Style::default().fg(Color::Magenta))
        .alignment(Alignment::Center);
    f.render_widget(status_bar, chunks[1]);
}

fn draw_submenu(f: &mut Frame, app: &App) {
    let event_date = app.event_date.as_deref().unwrap_or("<unknown date>");
    let title = format!("Participants à la maraude du {}", event_date);

    if app.attendees.is_empty() {
        let no_attendees = Paragraph::new("No attendees found.")
            .block(Block::default().title(title).borders(Borders::ALL))
            .alignment(Alignment::Center);
        f.render_widget(no_attendees, f.size());
        return;
    }

    // Create table rows for attendees
    let rows: Vec<Row> = app
    .attendees
    .iter()
    .enumerate()
    .map(|(row_idx, attendee)| {
        let first_name = attendee
            .profile
            .first_name
            .clone()
            .unwrap_or_default()
            .to_lowercase()
            .chars()
            .enumerate()
            .map(|(i, c)| if i == 0 { c.to_uppercase().to_string() } else { c.to_string() })
            .collect::<String>();

        let last_name = attendee
            .profile
            .last_name
            .clone()
            .unwrap_or_default()
            .to_lowercase()
            .chars()
            .enumerate()
            .map(|(i, c)| if i == 0 { c.to_uppercase().to_string() } else { c.to_string() })
            .collect::<String>();

        let email = attendee.profile.email.clone().unwrap_or_default().to_lowercase();

        let cells: Vec<Cell> = vec![
            Cell::from(first_name).style(if row_idx == app.selected_row && app.selected_col == 0 {
                Style::default().bg(Color::Magenta).fg(Color::White)
            } else {
                Style::default()
            }),
            Cell::from(last_name).style(if row_idx == app.selected_row && app.selected_col == 1 {
                Style::default().bg(Color::Magenta).fg(Color::White)
            } else {
                Style::default()
            }),
            Cell::from(email).style(if row_idx == app.selected_row && app.selected_col == 2 {
                Style::default().bg(Color::Magenta).fg(Color::White)
            } else {
                Style::default()
            }),
            Cell::from(attendee.profile.cell_phone.clone().unwrap_or_default())
                .style(if row_idx == app.selected_row && app.selected_col == 3 {
                    Style::default().bg(Color::Magenta).fg(Color::White)
                } else {
                    Style::default()
                }),
            Cell::from(attendee.birthdate.clone().unwrap_or_default())
                .style(if row_idx == app.selected_row && app.selected_col == 4 {
                    Style::default().bg(Color::Magenta).fg(Color::White)
                } else {
                    Style::default()
                }),
            Cell::from(attendee.ticket_class_name.clone().unwrap_or_default())
                .style(if row_idx == app.selected_row && app.selected_col == 5 {
                    Style::default().bg(Color::Magenta).fg(Color::White)
                } else {
                    Style::default()
                }),
            Cell::from(attendee.created.clone())
                .style(if row_idx == app.selected_row && app.selected_col == 6 {
                    Style::default().bg(Color::Magenta).fg(Color::White)
                } else {
                    Style::default()
                }),
        ];

        Row::new(cells)
    })
    .collect();
    let widths = [
        Constraint::Percentage(15),
        Constraint::Percentage(15),
        Constraint::Percentage(20),
        Constraint::Percentage(15),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
    ];

    // Create the table
    let table = Table::new(rows, widths)
        .header(
            Row::new(vec![
                Cell::from("Prénom"),
                Cell::from("Nom"),
                Cell::from("Email"),
                Cell::from("Téléphone"),
                Cell::from("Date de naissance"),
                Cell::from("Type d'inscription"),
                Cell::from("Date d'inscription"),
            ])
            .style(
                Style::default()
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(Block::default().title(title).borders(Borders::ALL))
        .widths(widths)
        .highlight_style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        );

    // Layout for the table and status bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Min(0),
                Constraint::Length(1),
            ]
            .as_ref(),
        )
        .split(f.size());

    // Render the table in the top chunk
    f.render_widget(table, chunks[0]);

    // Create and render the status bar in the bottom chunk
    let status_bar = Paragraph::new("Appuyez sur 'Échap' pour revenir au menu principal")
        .style(Style::default().fg(Color::Magenta))
        .alignment(Alignment::Center);
    f.render_widget(status_bar, chunks[1]);
}

fn draw_settings_menu(f: &mut Frame, app: &App) {
    // Charger la configuration actuelle
    let config = crate::eventbrite_auth::load_config().unwrap_or_default();

    // Préparer les éléments du menu
    let client_id_display = match &config.client_id {
        Some(client_id) => format!("Client ID EventBrite: {}", client_id),
        None => "Client ID EventBrite: <non défini>".to_string(),
    };

    let client_secret_display = match &config.client_secret {
        Some(_) => "Client Secret EventBrite: *****".to_string(),
        None => "Client Secret EventBrite: <non défini>".to_string(),
    };

    let items = vec![
        ListItem::new(client_id_display),
        ListItem::new(client_secret_display),
    ];

    // Mettre en surbrillance l'élément sélectionné
    let mut state = ListState::default();
    state.select(Some(app.settings_menu_index));

    // Créer le widget de liste
    let list = List::new(items)
        .block(Block::default().title("Paramétrage").borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    // Diviser l'écran en deux parties : liste et barre de statut
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Min(0), // La liste occupe l'espace restant
                Constraint::Length(1), // Hauteur de la barre de statut
            ]
            .as_ref(),
        )
        .split(f.size());

    // Rendre la liste dans la partie supérieure
    f.render_stateful_widget(list, chunks[0], &mut state);

    // Créer et rendre la barre de statut dans la partie inférieure
    let status_bar = Paragraph::new("Appuyez sur 'Échap' pour revenir au menu principal")
        .style(Style::default().fg(Color::Magenta))
        .alignment(Alignment::Center);
    f.render_widget(status_bar, chunks[1]);
}

fn draw_popup(f: &mut Frame, title: &str, input: &str) {
    let size = f.size();
    let popup_area = centered_rect(60, 20, size);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black).fg(Color::White));

    let paragraph = Paragraph::new(format!(
        "{}\n\nPress Enter to confirm or Esc to cancel",
        input
    ))
    .block(block)
    .alignment(Alignment::Center);

    f.render_widget(paragraph, popup_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, rect: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(rect);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}

fn draw_find_by_name_menu(f: &mut Frame, app: &App) {
    let state = &app.name_search_state;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(f.size());

    // Input fields
    let first_name = format!("Prénom: {}", state.input_first_name);
    let last_name = format!("Nom: {}", state.input_last_name);

    let first_name_paragraph = Paragraph::new(first_name)
        .block(Block::default().borders(Borders::ALL).title("Prénom"))
        .style(if state.focus == 0 { Style::default().fg(Color::Magenta) } else { Style::default() });
    let last_name_paragraph = Paragraph::new(last_name)
        .block(Block::default().borders(Borders::ALL).title("Nom"))
        .style(if state.focus == 1 { Style::default().fg(Color::Magenta) } else { Style::default() });

    f.render_widget(first_name_paragraph, chunks[0]);
    f.render_widget(last_name_paragraph, chunks[1]);

    // Results
    let results = if let Some(ref events) = state.results {
        if events.is_empty() {
            vec![ListItem::new("Aucun événement trouvé.")]
        } else {
            events.iter().map(|(event_name, event_date)| {
                ListItem::new(format!("{} ({})", event_name, event_date))
            }).collect()
        }
    } else {
        vec![ListItem::new("Entrer prénom et nom, puis valider.")]
    };

    let mut list_state = ListState::default();
    if state.focus == 2 && results.len() > 1 {
        list_state.select(Some(state.results_scroll));
    }

    let results_list = List::new(results)
        .block(Block::default().borders(Borders::ALL).title("Événements trouvés"))
        .highlight_style(Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD));

    f.render_stateful_widget(results_list, chunks[2], &mut list_state);

    // Status bar
    let status = if state.focus < 2 {
        "Entrer prénom/nom, Tab pour changer de champ, Entrée pour valider, Échap pour quitter"
    } else {
        "↑/↓ pour défiler, Échap pour revenir"
    };
    let status_bar = Paragraph::new(status)
        .style(Style::default().fg(Color::Magenta))
        .alignment(Alignment::Center);
    f.render_widget(status_bar, chunks[3]);
}
