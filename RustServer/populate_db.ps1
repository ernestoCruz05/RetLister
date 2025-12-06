# Script to populate the database with diverse leftover pieces (retalhos)

$SERVER_URL = "http://localhost:8000"

# Common materials used in workshops
$material = "MDF"
$thickness = 18

# Sample notes for context
$notes_samples = @(
    "Frente de gaveta",
    "Lateral armário",
    "Prateleira",
    "Tampo de mesa",
    "Rodapé",
    "Painel traseiro",
    "Porta pequena",
    "Reforço estrutural"
)

Write-Host "Populating database with 18mm MDF pieces for Nesting Test..." -ForegroundColor Cyan

# Generate 50 random pieces
for ($i = 1; $i -le 50; $i++) {
    # Random dimensions (realistic furniture parts)
    # Width: 100mm to 1200mm
    # Height: 100mm to 2000mm
    $width = Get-Random -Minimum 100 -Maximum 1200
    $height = Get-Random -Minimum 100 -Maximum 2000
    
    # Pick a random note
    $note_idx = Get-Random -Minimum 0 -Maximum $notes_samples.Count
    $notes = $notes_samples[$note_idx]

    $body = @{
        width_mm = $width
        height_mm = $height
        thickness_mm = $thickness
        material = $material
        notes = "$notes (Test Item $i)"
    } | ConvertTo-Json

    try {
        $response = Invoke-RestMethod -Uri "$SERVER_URL/add" -Method Post -ContentType "application/json" -Body $body
        Write-Host "[$i/50] Added: ${width}x${height}x${thickness}mm $material - ID: $($response.id)" -ForegroundColor Green
    } catch {
        Write-Host "[$i/50] Failed to add item: $_" -ForegroundColor Red
    }

    # Tiny delay to be nice to the local server
    Start-Sleep -Milliseconds 20
}

Write-Host "`nDone! Added 50 pieces of 18mm MDF." -ForegroundColor Yellow
Write-Host "Now go to the 'Otimizador' tab in the app and try to cut a list of MDF 18mm parts." -ForegroundColor White
