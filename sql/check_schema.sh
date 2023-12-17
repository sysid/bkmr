#!/bin/bash

# Database file to check
DB_FILE="$1"

# Expected schema elements
expected_tables=("bookmarks" "bookmarks_fts" "bookmarks_fts_data" "bookmarks_fts_idx" "bookmarks_fts_docsize" "bookmarks_fts_config" "__diesel_schema_migrations")
expected_triggers=("bookmarks_ai" "bookmarks_ad" "bookmarks_au" "UpdateLastTime")

# Function to check for table existence
check_table() {
    local table=$1
    local result=$(sqlite3 "$DB_FILE" "SELECT name FROM sqlite_master WHERE type='table' AND name='$table';")
    echo "-M- Checking table: $table"
    if [ -z "$result" ]; then
        echo "Missing table: $table"
        return 1
    fi
}

# Function to check for trigger existence
check_trigger() {
    local trigger=$1
    local result=$(sqlite3 "$DB_FILE" "SELECT name FROM sqlite_master WHERE type='trigger' AND name='$trigger';")
    echo "-M- Checking trigger: $trigger"
    if [ -z "$result" ]; then
        echo "Missing trigger: $trigger"
        return 1
    fi
}

# Check for each table
for table in "${expected_tables[@]}"; do
    check_table "$table" || exit 1
done

# Check for each trigger
for trigger in "${expected_triggers[@]}"; do
    check_trigger "$trigger" || exit 1
done

echo "All expected tables and triggers exist in the database."

