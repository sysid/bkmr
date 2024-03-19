import csv
import sqlite3

db_path = 'path/to/your/database.db'
csv_file_path = '/path/to/your/data.csv'

conn = sqlite3.connect(db_path)
cursor = conn.cursor()

with open(csv_file_path, 'r') as csv_file:
    reader = csv.DictReader(csv_file)  # Assuming the first row contains column names
    to_db = [(i['col1'], i['col2'], i['col3']) for i in reader]  # Adjust columns as necessary

cursor.executemany("INSERT INTO base_table (col1, col2, col3) VALUES (?, ?, ?);", to_db)
conn.commit()
conn.close()