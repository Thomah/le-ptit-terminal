import csv
import os
import sys
import time
import json
import requests
from datetime import datetime, timedelta

from sib_api_v3_sdk import ContactsApi, ApiClient, Configuration
from sib_api_v3_sdk.rest import ApiException

# --- CONFIGURATION ---


def load_config():
    config_path = os.path.join(
        os.environ["USERPROFILE"], ".les_ptits_gilets_config.json"
    )
    print(f"[DEBUG] Loading config from {config_path}")
    with open(config_path, "r", encoding="utf-8") as f:
        config = json.load(f)
    print(f"[DEBUG] Config loaded: keys={list(config.keys())}")
    return config


config = load_config()

# HelloAsso
HELLOASSO_CLIENT_ID = config["helloasso"]["client_id"]
HELLOASSO_CLIENT_SECRET = config["helloasso"]["client_secret"]
HELLOASSO_ORG_SLUG = config["helloasso"]["org_slug"]
HELLOASSO_FORM_TYPE = config["helloasso"].get("form_type", "membership")
HELLOASSO_FORM_SLUG = config["helloasso"]["form_slug"]

# Brevo (Sendinblue)
BREVO_API_KEY = config["brevo"]["api_key"]

# Output CSV
OUTPUT_CSV = "contacts_export.csv"

HELLOASSO_MEMBERS_FILE = "helloasso_members.json"
BREVO_CONTACTS_FILE = "brevo_contacts.json"
INTERMEDIATE_ROWS_FILE = "intermediate_rows.json"

def save_json(obj, path):
    with open(path, "w", encoding="utf-8") as f:
        json.dump(obj, f)

def load_json(path):
    with open(path, "r", encoding="utf-8") as f:
        return json.load(f)

def save_jsonl_row(row, path):
    with open(path, "a", encoding="utf-8") as f:
        f.write(json.dumps(row) + "\n")

def load_jsonl_rows(path):
    rows = []
    with open(path, "r", encoding="utf-8") as f:
        for line in f:
            rows.append(json.loads(line))
    return rows

def is_valid_email(email):
    import re

    return bool(re.match(r"^[^@\s]+@[^@\s]+\.[^@\s]+$", email))


# --- HELLOASSO AUTH ---


def get_helloasso_access_token(client_id, client_secret):
    url = "https://api.helloasso.com/oauth2/token"
    headers = {"content-type": "application/x-www-form-urlencoded"}
    data = {
        "grant_type": "client_credentials",
        "client_id": client_id,
        "client_secret": client_secret,
    }
    resp = requests.post(url, headers=headers, data=data)
    if resp.status_code != 200:
        raise Exception(f"Failed to get token: {resp.status_code} {resp.text}")
    token = resp.json()["access_token"]
    print(
        f"[DEBUG] Got HelloAsso access token: {token[:8]}..."
    )  # Only print first chars for security
    return token


# --- HELLOASSO SETUP ---


def get_helloasso_members(access_token):
    """
    Fetch all HelloAsso members for the given organization and form.
    Returns a list of dicts with keys: lastname, firstname, email, phone, membership_date, first_participation, comments
    """
    print(
        f"[DEBUG] Fetching HelloAsso members for org '{HELLOASSO_ORG_SLUG}' and form '{HELLOASSO_FORM_SLUG}'"
    )
    headers = {"Authorization": f"Bearer {access_token}"}
    members = []
    previous_token = None
    continuation_token = None
    per_page = 50
    while True:
        url = (
            f"https://api.helloasso.com/v5/organizations/{HELLOASSO_ORG_SLUG}/forms/"
            f"{HELLOASSO_FORM_TYPE}/{HELLOASSO_FORM_SLUG}/items"
            f"?pageSize={per_page}"
        )
        if continuation_token:
            url += f"&continuationToken={continuation_token}"
        print(f"[DEBUG] Requesting: {url}")
        resp = requests.get(url, headers=headers)
        if resp.status_code != 200:
            print(
                f"[ERROR] Failed to fetch HelloAsso members: {resp.status_code} {resp.text}",
                file=sys.stderr,
            )
            break
        data = resp.json()
        for item in data.get("data", []):
            payer = item.get("payer", {})
            order = item.get("order", {})
            lastname = payer.get("lastName", "").strip()
            firstname = payer.get("firstName", "").strip()
            email = payer.get("email", "").strip()
            date_adhesion = order.get("date", "")
            members.append(
                {
                    "lastname": lastname,
                    "firstname": firstname,
                    "email": email,
                    "date_adhesion": date_adhesion,
                }
            )
        previous_token = continuation_token
        continuation_token = data.get("pagination", {}).get("continuationToken")
        # Break if no token or token did not change
        if not continuation_token or continuation_token == previous_token:
            break
    print(f"[DEBUG] Retrieved {len(members)} HelloAsso members")
    return members


# --- BREVO SETUP ---


def get_brevo_api():
    print("[DEBUG] Initializing Brevo API client")
    configuration = Configuration()
    configuration.api_key["api-key"] = BREVO_API_KEY
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
        if isinstance(contact, str):
            try:
                contact = json.loads(contact)
            except Exception as e:
                print(f"[WARNING] Could not deserialize contact: {e}", file=sys.stderr)
                continue
        attributes = getattr(contact, "attributes", None)
        if attributes is None and isinstance(contact, dict):
            attributes = contact.get("attributes", None)
        fn = ln = ""
        if attributes:
            fn = attributes.get("FIRSTNAME", "").strip().lower()
            ln = attributes.get("LASTNAME", "").strip().lower()
        if not fn and hasattr(contact, "firstName"):
            fn = getattr(contact, "firstName", "").strip().lower()
        if not ln and hasattr(contact, "lastName"):
            ln = getattr(contact, "lastName", "").strip().lower()
        if not fn and isinstance(contact, dict):
            fn = contact.get("firstName", "").strip().lower()
        if not ln and isinstance(contact, dict):
            ln = contact.get("lastName", "").strip().lower()
        if not (fn and ln):
            print(
                f"[WARNING] Contact id={getattr(contact, 'id', contact.get('id', ''))} email={getattr(contact, 'email', contact.get('email', ''))} has no name info, skipping.",
                file=sys.stderr,
            )
            continue
        if fn == firstname.strip().lower() and ln == lastname.strip().lower():
            print(
                f"[DEBUG] Found contact by name: id={getattr(contact, 'id', contact.get('id', ''))}, email={getattr(contact, 'email', contact.get('email', ''))}"
            )
            found_ids.append(getattr(contact, "id", contact.get("id", "")))
            found_contacts.append(contact)
    return found_ids, found_contacts


# --- MAIN LOGIC ---


def main():
    # 1. Fetch or load HelloAsso members
    if os.path.exists(HELLOASSO_MEMBERS_FILE):
        print("[DEBUG] Loading HelloAsso members from file")
        members = load_json(HELLOASSO_MEMBERS_FILE)
    else:
        helloasso_access_token = get_helloasso_access_token(HELLOASSO_CLIENT_ID, HELLOASSO_CLIENT_SECRET)
        members = get_helloasso_members(helloasso_access_token)
        save_json(members, HELLOASSO_MEMBERS_FILE)

    # 2. Fetch or load Brevo contacts
    api = get_brevo_api()
    if os.path.exists(BREVO_CONTACTS_FILE):
        print("[DEBUG] Loading Brevo contacts from file")
        all_brevo_contacts = load_json(BREVO_CONTACTS_FILE)
    else:
        all_brevo_contacts = get_all_brevo_contacts(api)
        # If Brevo contacts are objects, convert to dicts for JSON
        all_brevo_contacts = [c if isinstance(c, dict) else c.__dict__ for c in all_brevo_contacts]
        save_json(all_brevo_contacts, BREVO_CONTACTS_FILE)

    # 3. Load already processed rows if any
    processed_keys = set()
    csv_rows = []
    if os.path.exists(INTERMEDIATE_ROWS_FILE):
        print("[DEBUG] Loading intermediate rows")
        for row in load_jsonl_rows(INTERMEDIATE_ROWS_FILE):
            key = (row["email"].lower(), row["firstname"].lower(), row["lastname"].lower())
            processed_keys.add(key)
            csv_rows.append(row)

    csv_rows = []
    for idx, member in enumerate(members, start=1):
        lastname = member["lastname"]
        firstname = member["firstname"]
        email = member["email"]
        date_adhesion = member["date_adhesion"]

        # Format DATE_ADHESION as "dd/MM/YYYY"
        formatted_date = ""
        if date_adhesion:
            try:
                dt = datetime.fromisoformat(date_adhesion.replace("Z", "+00:00"))
                formatted_date = dt.strftime("%d/%m/%Y")
            except Exception as e:
                print(f"[WARNING] Could not parse date '{date_adhesion}' for {firstname} {lastname}: {e}", file=sys.stderr)
                formatted_date = date_adhesion  # fallback

        print(f"[DEBUG] Processing member {idx}: {firstname} {lastname}, email={email}, date_adhesion={formatted_date}")

        ids_email, contacts_email = [], []
        if is_valid_email(email):
            ids_email, contacts_email = search_brevo_contact_by_email(api, email)
        else:
            print(f"[WARNING] Invalid email '{email}' for {firstname} {lastname} (member {idx}), skipping email lookup.", file=sys.stderr)

        ids_name, contacts_name = search_brevo_contact_by_name_cached(all_brevo_contacts, firstname, lastname)

        all_ids = set(ids_email) | set(ids_name)
        if len(all_ids) == 1:
            contact_id = list(all_ids)[0]
            print(f"[DEBUG] Unique contact found: {contact_id}")
        elif len(all_ids) > 1:
            print(f"[WARNING] Multiple contacts found for {firstname} {lastname} (member {idx})", file=sys.stderr)
            contact_id = "MULTIPLE"
        else:
            print(f"[DEBUG] No contact found for {firstname} {lastname}")
            contact_id = ""

        row = {
            "contact_id": contact_id,
            "email": email,
            "firstname": firstname,
            "lastname": lastname,
            "formatted_date": formatted_date,
            "ADHESION_OK": True
        }
        csv_rows.append(row)
        save_jsonl_row(row, INTERMEDIATE_ROWS_FILE)

    # Deduplicate by (email, firstname, lastname), keeping the latest DATE_ADHESION
    deduped = {}
    for row in csv_rows:
        key = (row["email"].lower(), row["firstname"].lower(), row["lastname"].lower())
        date_str = row["formatted_date"]
        try:
            date_obj = datetime.strptime(date_str, "%d/%m/%Y")
        except Exception:
            date_obj = None
        if key not in deduped or (
            date_obj and deduped[key]["date_obj"] and date_obj > deduped[key]["date_obj"]
        ):
            deduped[key] = {**row, "date_obj": date_obj}

    # Add ADHESION_OK column
    one_year_ago = datetime.now() - timedelta(days=365)
    for row in deduped.values():
        date_obj = row.get("date_obj")
        row["ADHESION_OK"] = bool(date_obj and date_obj > one_year_ago)

    # Write to CSV
    with open(OUTPUT_CSV, "w", newline='', encoding="utf-8") as csvfile:
        writer = csv.writer(csvfile)
        writer.writerow(["CONTACT ID", "EMAIL", "FIRSTNAME", "LASTNAME", "DATE_ADHESION", "ADHESION_OK"])
        for row in deduped.values():
            writer.writerow([
                row["contact_id"],
                row["email"],
                row["firstname"],
                row["lastname"],
                row["formatted_date"],
                str(row["ADHESION_OK"]).lower()
            ])

    print(f"[DEBUG] Exported to {OUTPUT_CSV}")


if __name__ == "__main__":
    main()
