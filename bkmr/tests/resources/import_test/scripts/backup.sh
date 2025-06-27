---
name: backup-database
tags: admin, backup, database
type: _shell_
---
#!/bin/bash
set -e

# Database backup script
DB_PATH="/data/app.db"
BACKUP_DIR="/backups"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

echo "Starting database backup..."
sqlite3 "$DB_PATH" ".backup $BACKUP_DIR/backup_$TIMESTAMP.db"
echo "Backup completed: $BACKUP_DIR/backup_$TIMESTAMP.db"
