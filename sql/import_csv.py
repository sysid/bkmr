import csv
import sqlite3

db_path = 'new.db'
csv_file_path = 'out.csv'

conn = sqlite3.connect(db_path)
cursor = conn.cursor()

# bkmr create-db new.db - creates a new database with one demo entry
with open(csv_file_path, 'r') as csv_file:
    reader = csv.DictReader(csv_file)  # Assuming the first row contains column names
    to_db = [(i+2, field['URL'], field['metadata'], field['tags'], field['desc'], field['flags'], field['last_update_ts']) for i, field in enumerate(reader)]  # Adjust columns as necessary

_ = None
cursor.executemany("INSERT INTO bookmarks (id, URL, metadata, tags, desc, flags, last_update_ts) VALUES (?, ?, ?, ?, ?, ?, ?);", to_db)
conn.commit()
conn.close()