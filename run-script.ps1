# Create a virtual environment in .venv
python -m venv .venv

# Activate the virtual environment
.venv\Scripts\Activate.ps1

# Upgrade pip
# python -m pip install --upgrade pip

# Install required dependencies
# pip install gspread google-auth google-auth-oauthlib sib-api-v3-sdk

# Run the script
python scripts\$($args[0]).py
