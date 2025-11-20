<#
.SYNOPSIS
    Automated backup script for RetLister SQLite database
.DESCRIPTION
    Creates daily and weekly backups with automatic cleanup
    Safe to run while server is running (WAL mode)
.NOTES
    Schedule with Task Scheduler:
    - Daily at 2 AM for daily backups
    - Sunday at 3 AM for weekly backups
#>

param(
    [string]$BackupType = "daily",  # "daily" or "weekly"
    [string]$DbPath = ".\data\retlister.db",
    [string]$BackupRoot = ".\data\backups"
)

$ErrorActionPreference = "Stop"

# Configuration
$DailyRetention = 7   # Keep 7 daily backups
$WeeklyRetention = 4  # Keep 4 weekly backups

# Create backup directories
$DailyDir = Join-Path $BackupRoot "daily"
$WeeklyDir = Join-Path $BackupRoot "weekly"
New-Item -ItemType Directory -Force -Path $DailyDir | Out-Null
New-Item -ItemType Directory -Force -Path $WeeklyDir | Out-Null

# Verify database exists
if (-not (Test-Path $DbPath)) {
    Write-Error "Database not found: $DbPath"
    exit 1
}

# Generate backup filename
$Timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
$BackupDir = if ($BackupType -eq "weekly") { $WeeklyDir } else { $DailyDir }
$BackupFile = Join-Path $BackupDir "retlister_${BackupType}_${Timestamp}.db"

Write-Host "Creating $BackupType backup..."
Write-Host "Source: $DbPath"
Write-Host "Destination: $BackupFile"

try {
    # Copy database files (WAL mode creates .db, .db-wal, .db-shm)
    Copy-Item $DbPath $BackupFile -Force
    
    # Also backup WAL and SHM if they exist
    $WalPath = "$DbPath-wal"
    $ShmPath = "$DbPath-shm"
    
    if (Test-Path $WalPath) {
        Copy-Item $WalPath "$BackupFile-wal" -Force
    }
    if (Test-Path $ShmPath) {
        Copy-Item $ShmPath "$BackupFile-shm" -Force
    }
    
    # Verify backup file size
    $OriginalSize = (Get-Item $DbPath).Length
    $BackupSize = (Get-Item $BackupFile).Length
    
    if ($BackupSize -eq 0) {
        throw "Backup file is empty!"
    }
    
    Write-Host "✓ Backup created successfully"
    Write-Host "  Original size: $([math]::Round($OriginalSize/1KB, 2)) KB"
    Write-Host "  Backup size: $([math]::Round($BackupSize/1KB, 2)) KB"
    
    # Cleanup old backups
    Write-Host "`nCleaning up old backups..."
    
    # Daily backups
    $DailyBackups = Get-ChildItem $DailyDir -Filter "retlister_daily_*.db" | 
                    Sort-Object LastWriteTime -Descending
    if ($DailyBackups.Count -gt $DailyRetention) {
        $ToDelete = $DailyBackups | Select-Object -Skip $DailyRetention
        foreach ($old in $ToDelete) {
            Remove-Item $old.FullName -Force
            Remove-Item "$($old.FullName)-wal" -Force -ErrorAction SilentlyContinue
            Remove-Item "$($old.FullName)-shm" -Force -ErrorAction SilentlyContinue
            Write-Host "  Deleted old daily backup: $($old.Name)"
        }
    }
    
    # Weekly backups
    $WeeklyBackups = Get-ChildItem $WeeklyDir -Filter "retlister_weekly_*.db" | 
                     Sort-Object LastWriteTime -Descending
    if ($WeeklyBackups.Count -gt $WeeklyRetention) {
        $ToDelete = $WeeklyBackups | Select-Object -Skip $WeeklyRetention
        foreach ($old in $ToDelete) {
            Remove-Item $old.FullName -Force
            Remove-Item "$($old.FullName)-wal" -Force -ErrorAction SilentlyContinue
            Remove-Item "$($old.FullName)-shm" -Force -ErrorAction SilentlyContinue
            Write-Host "  Deleted old weekly backup: $($old.Name)"
        }
    }
    
    Write-Host "`n✓ Backup complete"
    Write-Host "  Daily backups: $($DailyBackups.Count) / $DailyRetention"
    Write-Host "  Weekly backups: $($WeeklyBackups.Count) / $WeeklyRetention"
    
} catch {
    Write-Error "Backup failed: $_"
    exit 1
}
