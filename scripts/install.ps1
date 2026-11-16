$ErrorActionPreference = "Stop"

Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "        INSTALLING UNEVEN AI         " -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan

$Target = "win32-x64"

$InstallDir = "$env:USERPROFILE\.uneven\bin"
if (-not (Test-Path -Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

# Clean up legacy binaries and companion modules from older versions
$LegacyFiles = @(
    "$InstallDir\uneven-ai.exe",
    "$InstallDir\uneven-core.uneven",
    "$InstallDir\uneven_core.node",
    "$InstallDir\uneven.uneven"
)
foreach ($File in $LegacyFiles) {
    if (Test-Path -Path $File) {
        Write-Host "-> Cleaning up legacy file: $(Split-Path $File -Leaf)..."
        Remove-Item -Path $File -Force | Out-Null
    }
}

$ExeDest = "$InstallDir\uneven.exe"
$DownloadBase = "https://edge.uneven.dev/api/download"

Write-Host "-> Fetching binary for $Target..."
Invoke-WebRequest -Uri "$DownloadBase?target=$Target&asset=bin" -OutFile $ExeDest

# Setup User PATH
$UserPath = [Environment]::GetEnvironmentVariable("PATH", "User")
if ($UserPath -notlike "*$InstallDir*") {
    Write-Host "-> Adding $InstallDir to user PATH..."
    $NewPath = "$InstallDir;" + $UserPath
    [Environment]::SetEnvironmentVariable("PATH", $NewPath, "User")
    $env:PATH = "$InstallDir;" + $env:PATH
}

Write-Host "==================================================" -ForegroundColor Cyan
Write-Host " ✨ Installation complete! Start a new terminal  " -ForegroundColor Green
Write-Host "    Then run: uneven init                      " -ForegroundColor Green
Write-Host "==================================================" -ForegroundColor Cyan
