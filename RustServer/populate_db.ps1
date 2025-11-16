# Script to populate the database with diverse leftover pieces (retalhos)

$SERVER_URL = "http://localhost:8000"

# Common materials used in workshops
$materials = @("MDF", "Contraplacado", "Aglomerado", "OSB", "Melamina", "Madeira Maciça", "Forro", "HDF")

# Common thicknesses in mm
$thicknesses = @(3, 6, 9, 12, 15, 18, 19, 22, 25, 30)

# Sample notes
$notes_samples = @(
    "Bom estado",
    "Pequeno risco no canto",
    "Superficie limpa",
    "Resto de corte diagonal",
    "Bordos irregulares",
    "",
    "Pronto a usar",
    "Verificar medidas",
    "Peça de qualidade",
    "Sobra de projeto anterior"
)

Write-Host "Populating database with diverse retalhos..." -ForegroundColor Green

# Generate 50 diverse leftover pieces
for ($i = 1; $i -le 50; $i++) {
    # Random dimensions (realistic workshop leftovers)
    $width = Get-Random -Minimum 100 -Maximum 2400
    $height = Get-Random -Minimum 80 -Maximum 1800
    $thickness = $thicknesses | Get-Random
    $material = $materials | Get-Random
    $notes = $notes_samples | Get-Random

    $body = @{
        width_mm = $width
        height_mm = $height
        thickness_mm = $thickness
        material = $material
        notes = if ($notes -eq "") { $null } else { $notes }
    } | ConvertTo-Json

    try {
        $response = Invoke-RestMethod -Uri "$SERVER_URL/add" -Method Post -ContentType "application/json" -Body $body
        Write-Host "[$i/50] Added: ${width}x${height}x${thickness}mm $material (ID: $($response.id))" -ForegroundColor Cyan
    } catch {
        Write-Host "[$i/50] Failed to add resto: $_" -ForegroundColor Red
    }

    # Small delay to avoid overwhelming the server
    Start-Sleep -Milliseconds 50
}

Write-Host "`nDone! Added 50 diverse retalhos to the database." -ForegroundColor Green
Write-Host "Run 'Invoke-RestMethod -Uri $SERVER_URL/stats' to see statistics." -ForegroundColor Yellow
