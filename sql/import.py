import sqlite3

def import_table_from_file(db_path: str, table_name: str, input_file_path: str):
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()

    # Open the input file
    with open(input_file_path, 'r') as file:
        for line in file:
            # Split the line back into columns
            columns = line.strip().split('|')

            # Decode hex-encoded BLOB data
            columns[-2] = bytes.fromhex(columns[-2])  # Assuming embedding is the second last column
            columns[-1] = bytes.fromhex(columns[-1])  # Assuming content_hash is the last column

            # Insert the data into the new table
            cursor.execute(f"INSERT INTO {table_name} VALUES ({','.join('?' for _ in columns)})", columns)

    # Commit the changes and cleanup
    conn.commit()
    cursor.close()
    conn.close()

# Example usage
import_table_from_file('new_bm.db', 'bookmarks', 'bookmarks_export.txt')
