$ErrorActionPreference = 'Stop'
Push-Location $PSScriptRoot
try {
    cargo build --locked --release --bin Yami
    if ($LASTEXITCODE -ne 0) { throw 'Application build failed.' }
    cargo build --locked --release --features installer --bin Yami-Setup
    if ($LASTEXITCODE -ne 0) { throw 'Installer build failed.' }
    Write-Output 'Built target\release\Yami.exe and target\release\Yami-Setup.exe'
} finally { Pop-Location }
