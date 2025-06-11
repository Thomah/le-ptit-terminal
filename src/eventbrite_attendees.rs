use log::{debug, error};
use reqwest::blocking::Client;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Organization {
    id: String,
}

#[derive(Debug, Deserialize)]
struct OrganizationsResponse {
    organizations: Vec<Organization>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Event {
    pub id: String,
    pub name: EventName,
    pub start: EventStart,
}

#[derive(Debug, Deserialize)]
pub struct EventStart {
    pub local: String
}

#[derive(Debug, Deserialize)]
pub struct EventName {
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct EventsResponse {
    pub events: Vec<Event>,
}

#[derive(Debug, Deserialize)]
struct AttendeeResponse {
    attendees: Vec<Attendee>,
    pagination: Pagination,
}

#[derive(Debug, Deserialize)]
struct Pagination {
    has_more_items: bool,
}

#[derive(Debug, Deserialize)]
pub struct Attendee {
    pub profile: AttendeeProfile,
    pub created: String,
    pub ticket_class_name: Option<String>,
    pub birthdate: Option<String>,
    answers: Option<Vec<Answer>>
}

#[derive(Debug, Deserialize)]
pub struct AttendeeProfile {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub cell_phone: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Answer {
    question: String,
    #[serde(default)]
    answer: Option<String>,
}

pub fn get_attendees_from_api(token: &str) -> Result<(Vec<Attendee>, String), anyhow::Error> {
    let client = Client::new();

    debug!("Starting to fetch organization ID...");
    let org_id = get_organization_id(&client, token)
        .ok_or_else(|| anyhow::anyhow!("Failed to fetch organization ID"))?;
    debug!("Fetched organization ID: {}", org_id);

    debug!("Starting to fetch the next event...");
    let event = get_next_event(&client, token, &org_id)
        .ok_or_else(|| anyhow::anyhow!("Failed to fetch next event"))?;
    debug!("Fetched next event: {} ({})", event.name.text, event.id);

    debug!("Starting to fetch attendees for event ID: {}", event.id);
    let mut attendees = get_attendees(&client, token, &event.id);
    debug!("Fetched {} attendees for event ID: {}", attendees.len(), event.id);

    // Sort attendees
    attendees.sort_by(|a, b| {
        let ticket_class_a = a.ticket_class_name.as_deref().unwrap_or("");
        let ticket_class_b = b.ticket_class_name.as_deref().unwrap_or("");

        // "Liste Principale" first
        if ticket_class_a == "Liste Principale" && ticket_class_b != "Liste Principale" {
            return std::cmp::Ordering::Less;
        }
        if ticket_class_b == "Liste Principale" && ticket_class_a != "Liste Principale" {
            return std::cmp::Ordering::Greater;
        }

        // Then by created date (descending)
        b.created.cmp(&a.created)
    });

    // Parse and format the event's date
    let event_date = chrono::NaiveDateTime::parse_from_str(&event.start.local, "%Y-%m-%dT%H:%M:%S")
        .map(|dt| dt.format("%d/%m/%Y").to_string())
        .unwrap_or_else(|_| "<invalid date>".to_string());

    Ok((attendees, event_date))
}

pub fn get_organization_id(client: &Client, token: &str) -> Option<String> {
    debug!("Fetching organization ID...");
    let res = client
        .get("https://www.eventbriteapi.com/v3/users/me/organizations/")
        .bearer_auth(token)
        .send()
        .ok()?;

    if !res.status().is_success() {
        error!("Failed to fetch organization ID: {:?}", res.text().ok()?);
        return None;
    }

    let data: OrganizationsResponse = res.json().ok()?;
    let org_id = data.organizations.first().map(|org| org.id.clone());
    debug!("Organization ID fetched: {:?}", org_id);
    org_id
}

pub fn get_next_event(client: &Client, token: &str, org_id: &str) -> Option<Event> {
    debug!("Fetching next event for organization ID: {}", org_id);
    let res = client
        .get(&format!(
            "https://www.eventbriteapi.com/v3/organizations/{}/events/",
            org_id
        ))
        .bearer_auth(token)
        .query(&[("order_by", "start_asc"), ("status", "live")])
        .send()
        .ok()?;

    if !res.status().is_success() {
        error!("Failed to fetch events: {:?}", res.text().ok()?);
        return None;
    }

    let data: EventsResponse = res.json().ok()?;
    let next_event = data.events.into_iter().next();
    debug!("Next event fetched: {:?}", next_event);
    next_event
}

pub fn get_attendees(client: &Client, token: &str, event_id: &str) -> Vec<Attendee> {
    debug!("Fetching attendees for event ID: {}", event_id);
    let mut attendees = vec![];
    let mut page = 1;

    loop {
        debug!("Fetching attendees, page: {}", page);
        let res = client
            .get(&format!(
                "https://www.eventbriteapi.com/v3/events/{}/attendees/",
                event_id
            ))
            .bearer_auth(token)
            .query(&[("page", page.to_string())])
            .send();

        match res {
            Ok(response) => {
                if !response.status().is_success() {
                    error!(
                        "Failed to fetch attendees for page: {}. Status: {}. Body: {:?}",
                        page,
                        response.status(),
                        response.text().ok()
                    );
                    break;
                }

                let raw_body = response.text().unwrap_or_else(|_| "Failed to read body".to_string());
                debug!("Raw response body for page {}: {}", page, raw_body);

                let mut data = match serde_json::from_str::<AttendeeResponse>(&raw_body) {
                    Ok(parsed) => parsed,
                    Err(err) => {
                        error!(
                            "Failed to parse attendees response for page: {}. Error: {}. Body: {}",
                            page, err, raw_body
                        );
                        break;
                    }
                };

                // Extract birthdate from answers
                for attendee in &mut data.attendees {
                    if let Some(answers) = &attendee.answers {
                        attendee.birthdate = answers
                            .iter()
                            .find(|answer| answer.question.to_lowercase() == "date de naissance")
                            .and_then(|answer| answer.answer.clone());
                    }
                }

                debug!(
                    "Fetched {} attendees from page: {}",
                    data.attendees.len(),
                    page
                );
                attendees.extend(data.attendees);

                if !data.pagination.has_more_items {
                    debug!("No more pages of attendees to fetch.");
                    break;
                }

                page += 1;
            }
            Err(err) => {
                error!("Failed to fetch attendees for page: {}. Error: {}", page, err);
                break;
            }
        }
    }

    debug!("Total attendees fetched: {}", attendees.len());
    attendees
}
