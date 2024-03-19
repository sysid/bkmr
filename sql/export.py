import sqlite3

"""
export the data to a format that can faithfully represent BLOB data. 
One way is to use a hex representation for the BLOB fields during the export. 
You can then convert back from hex to BLOB when re-importing.

Here's a Python script that demonstrates how to export the data from the bookmarks table, including handling BLOB fields. 
This script exports the data into a custom format where textual data is plain text, but BLOB data is hex-encoded. 
For simplicity, this example exports to a simple text file, 
but you can modify it for other formats or even to encode everything in base64 or another binary-safe encoding.
"""

def export_table_to_file(db_path: str, table_name: str, output_file_path: str):
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()

    # Selecting all data from the table
    cursor.execute(f"SELECT *, hex(embedding), hex(content_hash) FROM {table_name}")

    # Open the output file
    with open(output_file_path, 'w') as file:
        for row in cursor.fetchall():
            # Converting the row to a string with proper encoding for BLOB fields
            row_str = '|'.join(str(item) for item in row)  # Use a separator that does not conflict with your data
            file.write(row_str + '\n')

    # Cleanup
    cursor.close()
    conn.close()

# Example usage
export_table_to_file('bm.db', 'bookmarks', 'bookmarks_export.txt')
