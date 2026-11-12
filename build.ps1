# Bygger TRK ForceShot 2 för Windows och lägger två filer i dist:
#
#   TRK ForceShot.exe                 programmet, att köra som det är
#   TRK ForceShot Setup <ver>.exe     samma fil, men installerar sig själv när
#                                     den körs. Det är den man skickar vidare.
#
#   .\build.ps1                  bygger med versionen som står i Cargo.toml
#   .\build.ps1 -Version 2.2.2   stämplar en ny version först
#   .\build.ps1 -Test            kör dessutom enhetstesterna
#
# Kräver Rust (rustup).
[CmdletBinding()]
param(
    [ValidatePattern('^\d+\.\d+\.\d+$')]
    [string]$Version,
    [switch]$Test
)

$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
Set-Location $root

if ($Version) {
    # Första version-raden i Cargo.toml är paketets egen.
    $toml = Join-Path $root 'Cargo.toml'
    $lines = [System.IO.File]::ReadAllLines($toml)
    for ($i = 0; $i -lt $lines.Length; $i++) {
        if ($lines[$i] -match '^version\s*=') { $lines[$i] = "version = `"$Version`""; break }
    }
    [System.IO.File]::WriteAllLines($toml, $lines, (New-Object System.Text.UTF8Encoding $false))
    Write-Host "  Version $Version" -ForegroundColor Yellow
}

if ($Test) {
    Write-Host '  Kör tester...' -ForegroundColor Yellow
    cargo test --quiet
    if ($LASTEXITCODE -ne 0) { exit 1 }
}

# Sökvägar på den egna datorn (användarnamn, mappar) ska inte följa med in i programfilen.
# (Kodad variant med skiljetecknet 0x1F, eftersom sökvägarna kan innehålla mellanslag.)
$env:CARGO_ENCODED_RUSTFLAGS = "--remap-path-prefix=$root=forceshot" + [char]0x1f + "--remap-path-prefix=$env:USERPROFILE=~"

Write-Host '  Kompilerar...' -ForegroundColor Yellow
cargo build --release
if ($LASTEXITCODE -ne 0) { exit 1 }

$meta = cargo metadata --format-version 1 --no-deps | ConvertFrom-Json
$built = Join-Path $meta.target_directory 'release\TRK-ForceShot.exe'
$ver = ($meta.packages | Where-Object { $_.name -eq 'forceshot' }).version

$dist = Join-Path $root 'dist'
New-Item -ItemType Directory -Force $dist | Out-Null
$exe = Join-Path $dist 'TRK ForceShot.exe'
Copy-Item $built $exe -Force

# Installationsfilen är samma binär med ett annat namn: programmet visar
# installationsfönstret när filnamnet innehåller "Setup".
Get-ChildItem $dist -Filter 'TRK ForceShot Setup *.exe' -ErrorAction SilentlyContinue | Remove-Item -Force
$setup = Join-Path $dist "TRK ForceShot Setup $ver.exe"
Copy-Item $exe $setup -Force

$size = [Math]::Round((Get-Item $exe).Length / 1MB, 1)
Write-Host "  Klart:  $exe" -ForegroundColor Green
Write-Host "  Version $ver · $size MB"
Write-Host "  Att skicka vidare:  $setup" -ForegroundColor Green
Write-Host ''
Write-Host '  Ny utgåva: skapa en release på GitHub med taggen' "v$ver" 'och bifoga Setup-filen.'
