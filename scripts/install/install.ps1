# Alius CLI Installer for Windows
# Usage: irm https://raw.githubusercontent.com/AliusTech/alius/main/scripts/install/install.ps1 | iex

$ErrorActionPreference = "Stop"

$REPO = "AliusTech/alius"
$BINARY_NAME = "alius"

function Write-Info {
    param([string]$Message)
    Write-Host $Message -ForegroundColor Blue
}

function Write-Warn {
    param([string]$Message)
    Write-Host $Message -ForegroundColor Yellow
}

function Write-Error-Custom {
    param([string]$Message)
    Write-Host $Message -ForegroundColor Red
    exit 1
}

function Write-Success {
    param([string]$Message)
    Write-Host $Message -ForegroundColor Green
}

function Detect-Architecture {
    try {
        $arch = [System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture
        switch ($arch) {
            "X64" { return "x64" }
            "Arm64" { return "arm64" }
            default { Write-Error-Custom "Unsupported architecture: $arch" }
        }
    } catch {
        # Fallback for older PowerShell versions
        $envArch = $env:PROCESSOR_ARCHITECTURE
        switch ($envArch) {
            "AMD64" { return "x64" }
            "ARM64" { return "arm64" }
            default { Write-Error-Custom "Unsupported architecture: $envArch" }
        }
    }
}

function Resolve-Artifact {
    param([string]$Arch)
    return "alius-windows-$Arch.zip"
}

function Fetch-LatestVersion {
    $url = "https://api.github.com/repos/$REPO/releases/latest"
    try {
        $response = Invoke-RestMethod -Uri $url -Method Get
        $tag = $response.tag_name
        # Remove 'v' prefix if present
        return $tag -replace '^v', ''
    } catch {
        Write-Error-Custom "Failed to fetch latest version from GitHub: $_"
    }
}

function Download-File {
    param([string]$Url, [string]$Destination)
    try {
        Invoke-WebRequest -Uri $Url -OutFile $Destination -UseBasicParsing
    } catch {
        Write-Error-Custom "Download failed: $Url - $_"
    }
}

function Show-Usage {
    Write-Host @"
Alius CLI Installer for Windows

Usage:
  .\install.ps1 [OPTIONS]

Options:
  -Version VERSION      Install specific version (e.g., 0.6.15)
  -BinDir DIR           Installation directory (default: ~\.alius\bin)
  -Yes                  Skip confirmation prompt
  -Help                 Show this help message

Environment Variables:
  ALIUS_VERSION         Version to install (alternative to -Version)
  ALIUS_INSTALL_DIR     Installation directory (alternative to -BinDir)

Examples:
  # Install latest version
  irm https://raw.githubusercontent.com/AliusTech/alius/main/scripts/install/install.ps1 | iex

  # Install specific version
  .\install.ps1 -Version 0.6.15

  # Install to custom directory
  .\install.ps1 -BinDir C:\Tools\bin
"@
    exit 0
}

function Get-InstallDir {
    if ($env:ALIUS_INSTALL_DIR) {
        return $env:ALIUS_INSTALL_DIR
    }
    return "$env:USERPROFILE\.alius\bin"
}

function Add-ToPath {
    param([string]$Dir)
    $currentPath = [Environment]::GetEnvironmentVariable("PATH", "User")
    if ($currentPath -notlike "*$Dir*") {
        $newPath = "$Dir;$currentPath"
        [Environment]::SetEnvironmentVariable("PATH", $newPath, "User")
        $env:PATH = "$Dir;$env:PATH"
        Write-Info "Added $Dir to user PATH"
    }
}

# Parse arguments
param(
    [string]$Version,
    [string]$BinDir,
    [switch]$Yes,
    [switch]$Help
)

if ($Help) {
    Show-Usage
}

Write-Info "Installing Alius CLI..."

# Detect architecture
$arch = Detect-Architecture
$artifact = Resolve-Artifact -Arch $arch

Write-Info "Platform: windows-$arch"

# Determine version
if ($Version) {
    $version = $Version
} elseif ($env:ALIUS_VERSION) {
    $version = $env:ALIUS_VERSION
} else {
    Write-Info "Fetching latest version..."
    $version = Fetch-LatestVersion
}

Write-Info "Version: $version"

# Construct download URL
$url = "https://github.com/$REPO/releases/download/v$version/$artifact"
Write-Info "Download URL: $url"

# Create temp directory
$tmpDir = Join-Path $env:TEMP "alius-install-$(Get-Random)"
New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null

try {
    # Download artifact
    Write-Info "Downloading $artifact..."
    $archivePath = Join-Path $tmpDir $artifact
    Download-File -Url $url -Destination $archivePath

    # Extract artifact
    Write-Info "Extracting..."
    Expand-Archive -Path $archivePath -DestinationPath $tmpDir -Force

    # Determine install directory
    if ($BinDir) {
        $installDir = $BinDir
    } else {
        $installDir = Get-InstallDir
    }
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null

    # Install binary
    Write-Info "Installing to $installDir..."
    $sourceBinary = Join-Path $tmpDir "$BINARY_NAME.exe"
    $destBinary = Join-Path $installDir "$BINARY_NAME.exe"
    Copy-Item -Path $sourceBinary -Destination $destBinary -Force

    # Add to PATH
    Add-ToPath -Dir $installDir

    # Verify installation
    Write-Info "Verifying installation..."
    $installedVersion = & $destBinary --version 2>&1 | Select-Object -First 1
    Write-Success "Alius CLI installed successfully!"
    Write-Info "Version: $installedVersion"
    Write-Info "Location: $destBinary"
    Write-Info ""
    Write-Info "You may need to restart your terminal for PATH changes to take effect."
} finally {
    # Cleanup
    Remove-Item -Path $tmpDir -Recurse -Force -ErrorAction SilentlyContinue
}
