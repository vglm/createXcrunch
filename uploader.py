import os
from time import sleep

import requests
import glob

from exceptiongroup import catch

# Configuration
FOLDER_PATH = 'output'  # Path to the folder containing the CSV files
API_URL = 'https://addressology.ovh/api/fancy/new'  # API endpoint URL

def upload_file(salt, address, factory, version):
    """Uploads a file to the specified API endpoint."""
    response = requests.post(API_URL, json={'salt': salt, 'address': address, 'factory': factory, 'miner': version})
    if response.status_code == 200:
        print(f"Successfully uploaded {address}")
        return True
    else:
        print(f"Failed to upload {address}. Status code: {response.status_code}")
        return False

def main():
    while True:
        try:

            # Get all CSV files in the folder with the naming pattern addr_1_2.csv
            csv_files = glob.glob(os.path.join(FOLDER_PATH, 'addr_*.csv'))

            if not csv_files:
                print("No CSV files found in the specified folder.")
            else:
                for csv_file in csv_files:
                    with open(csv_file, 'r') as f:
                        data = f.read()

                    salt = data.split(',')[0]
                    address = data.split(',')[1]
                    factory = data.split(',')[2]
                    version = data.split(',')[3]

                    if upload_file(salt, address, factory, version):
                        # If the upload was successful, delete the file
                        os.remove(csv_file)
        except Exception as e:
            print(f"An error occurred: {e}")

        sleep(5)

if __name__ == "__main__":
    main()