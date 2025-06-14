import csv
import os
import sys
import time
import json
import tempfile
import re

import gspread
from google.oauth2.service_account import Credentials
from sib_api_v3_sdk import ContactsApi, ApiClient, Configuration
from sib_api_v3_sdk.rest import ApiException

# --- CONFIGURATION ---

def load_config():
    config_path = os.path.join(os.environ["USERPROFILE"], ".les_ptits_gilets_config.json")
    print(f"[DEBUG] Loading config from {config_path}")
    with open(config_path, "r", encoding="utf-8") as f:
        config = json.load(f)
    print(f"[DEBUG] Config loaded: keys={list(config.keys())}")
    return config

config = load_config()

# Google Sheets
GOOGLE_SHEET_ID = config["google"]["volunteer_list"]["sheet_id"]
GOOGLE_SHEET_RANGE = config["google"]["volunteer_list"]["sheet_range"]
GOOGLE_CREDENTIALS_DICT = config["google"]["credentials"]

# Write the credentials to a hidden temp file for google-auth
temp_dir = os.path.join(os.environ["USERPROFILE"], ".les_ptits_gilets")
os.makedirs(temp_dir, exist_ok=True)
GOOGLE_CREDENTIALS_FILE = os.path.join(temp_dir, "google-credentials.json")
with open(GOOGLE_CREDENTIALS_FILE, "w", encoding="utf-8") as f:
    json.dump(GOOGLE_CREDENTIALS_DICT, f)
print(f"[DEBUG] Google credentials written to {GOOGLE_CREDENTIALS_FILE}")

# Brevo (Sendinblue)
BREVO_API_KEY = config["brevo"]["api_key"]

# Output CSV
OUTPUT_CSV = "contacts_export.csv"

def is_valid_email(email):
    # Simple regex for email validation
    return bool(re.match(r"^[^@\s]+@[^@\s]+\.[^@\s]+$", email))

# --- GOOGLE SHEETS SETUP ---

def get_google_sheet_rows():
    print(f"[DEBUG] Connecting to Google Sheets with ID: {GOOGLE_SHEET_ID}, Range: {GOOGLE_SHEET_RANGE}")
    scopes = [
        "https://www.googleapis.com/auth/spreadsheets.readonly",
        "https://www.googleapis.com/auth/drive.readonly"
    ]
    creds = Credentials.from_service_account_file(GOOGLE_CREDENTIALS_FILE, scopes=scopes)
    gc = gspread.authorize(creds)
    sh = gc.open_by_key(GOOGLE_SHEET_ID)
    worksheet = sh.worksheet(GOOGLE_SHEET_RANGE)
    rows = worksheet.get_all_values()
    print(f"[DEBUG] Retrieved {len(rows)} rows from Google Sheet")
    return rows[7:]  # Skip first 7 lines

# --- BREVO SETUP ---

def get_brevo_api():
    print("[DEBUG] Initializing Brevo API client")
    configuration = Configuration()
    configuration.api_key['api-key'] = BREVO_API_KEY
    return ContactsApi(ApiClient(configuration))

def get_all_brevo_contacts(api):
    """Download all Brevo contacts and return as a list."""
    all_contacts = []
    limit = 50
    offset = 0
    print("[DEBUG] Downloading all Brevo contacts...")
    while True:
        print(f"[DEBUG] Fetching Brevo contacts batch: offset={offset}, limit={limit}")
        resp = api.get_contacts(limit=limit, offset=offset)
        all_contacts.extend(resp.contacts)
        if len(resp.contacts) < limit:
            break
        offset += limit
    print(f"[DEBUG] Downloaded {len(all_contacts)} contacts from Brevo.")
    return all_contacts

def search_brevo_contact_by_email(api, email):
    ids = []
    contacts = []
    if email:
        try:
            resp = api.get_contact_info(email)
            print(f"[DEBUG] Found contact by email: id={resp.id}")
            ids.append(resp.id)
            contacts.append(resp)
        except ApiException as e:
            if e.status != 404:
                print(f"[ERROR] Error searching by email {email}: {e}", file=sys.stderr)
            else:
                print(f"[DEBUG] No contact found by email: {email}")
    return ids, contacts

def search_brevo_contact_by_name_cached(all_contacts, firstname, lastname):
    found_ids = []
    found_contacts = []
    for contact in all_contacts:
        # print(f"[DEBUG] Raw contact: {contact}")
        if isinstance(contact, str):
            try:
                contact = json.loads(contact)
            except Exception as e:
                print(f"[WARNING] Could not deserialize contact: {e}", file=sys.stderr)
                continue
        attributes = getattr(contact, "attributes", None)
        if attributes is None and isinstance(contact, dict):
            attributes = contact.get("attributes", None)
        if not attributes:
            # print(f"[WARNING] Contact id={getattr(contact, 'id', contact.get('id', ''))} email={getattr(contact, 'email', contact.get('email', ''))} has no attributes, skipping.", file=sys.stderr)
            continue
        fn = attributes.get("FIRSTNAME", "").strip().lower()
        ln = attributes.get("LASTNAME", "").strip().lower()
        if fn == firstname.strip().lower() and ln == lastname.strip().lower():
            print(f"[DEBUG] Found contact by name: id={getattr(contact, 'id', contact.get('id', ''))}, email={getattr(contact, 'email', contact.get('email', ''))}")
            found_ids.append(getattr(contact, "id", contact.get("id", "")))
            found_contacts.append(contact)
    return found_ids, found_contacts

# --- MAIN LOGIC ---

def main():
    if not BREVO_API_KEY:
        print("Please set the BREVO_API_KEY environment variable.", file=sys.stderr)
        sys.exit(1)

    print("[DEBUG] Starting main process")
    rows = get_google_sheet_rows()
    api = get_brevo_api()

    # Download all Brevo contacts once
    all_brevo_contacts = get_all_brevo_contacts(api)

    with open(OUTPUT_CSV, "w", newline='', encoding="utf-8") as csvfile:
        writer = csv.writer(csvfile)
        writer.writerow(["CONTACT ID", "EMAIL", "FIRSTNAME", "LASTNAME", "SMS"])

        for idx, row in enumerate(rows, start=8):
            lastname = row[0].strip()
            firstname = row[1].strip()
            email = row[2].strip()
            sms = row[3].strip()
            print(f"[DEBUG] Processing row {idx}: {firstname} {lastname}, email={email}, sms={sms}")

            # Validate email before querying Brevo
            ids_email, contacts_email = [], []
            if is_valid_email(email):
                ids_email, contacts_email = search_brevo_contact_by_email(api, email)
            else:
                print(f"[WARNING] Invalid email '{email}' for {firstname} {lastname} (row {idx}), skipping email lookup.", file=sys.stderr)

            # Use cached contacts for name search
            ids_name, contacts_name = search_brevo_contact_by_name_cached(all_brevo_contacts, firstname, lastname)

            # Merge and deduplicate IDs
            all_ids = set(ids_email) | set(ids_name)
            if len(all_ids) == 1:
                contact_id = list(all_ids)[0]
                print(f"[DEBUG] Unique contact found: {contact_id}")
            elif len(all_ids) > 1:
                print(f"[WARNING] Multiple contacts found for {firstname} {lastname} (row {idx})", file=sys.stderr)
                contact_id = "MULTIPLE"
            else:
                print(f"[DEBUG] No contact found for {firstname} {lastname}")
                contact_id = ""

            writer.writerow([contact_id, email, firstname, lastname, sms])
            time.sleep(0.2)  # Avoid hitting API rate limits

    print(f"[DEBUG] Exported to {OUTPUT_CSV}")

if __name__ == "__main__":
    main()