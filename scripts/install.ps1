# mailbox-mcp installer for Windows
# Usage: iwr -useb https://raw.githubusercontent.com/siy/mailbox-mcp/master/scripts/install.ps1 | iex

$ErrorActionPreference = "Stop"

$Repo = "siy/mailbox-mcp"
$BinaryName = "mailbox-mcp"
$InstallDir = "$env:USERPROFILE\.local\bin"

function Get-Architecture {
    $arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
    switch ($arch) {
        "X64" { return "x86_64" }
        "Arm64" { return "aarch64" }
        default {
            Write-Error "Unsupported architecture: $arch"
            exit 1
        }
    }
}

function Get-LatestRelease {
    $response = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -UseBasicParsing
    return $response.tag_name
}

function Install-MailboxMcp {
    $arch = Get-Architecture
    $target = "$arch-pc-windows-msvc"

    Write-Host "Detecting system..."
    Write-Host "  Architecture: $arch"
    Write-Host "  Target: $target"

    Write-Host "Fetching latest release..."
    $version = Get-LatestRelease
    if (-not $version) {
        Write-Error "Could not determine latest version"
        exit 1
    }
    Write-Host "  Version: $version"

    $archiveName = "$BinaryName-$target.zip"
    $downloadUrl = "https://github.com/$Repo/releases/download/$version/$archiveName"

    Write-Host "Downloading $archiveName..."

    $tempDir = New-TemporaryFile | ForEach-Object { Remove-Item $_; New-Item -ItemType Directory -Path $_ }
    $archivePath = Join-Path $tempDir $archiveName

    try {
        Invoke-WebRequest -Uri $downloadUrl -OutFile $archivePath -UseBasicParsing

        Write-Host "Extracting..."
        Expand-Archive -Path $archivePath -DestinationPath $tempDir -Force

        Write-Host "Installing to $InstallDir..."
        if (-not (Test-Path $InstallDir)) {
            New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
        }

        # Find binary - may be at root or in subdirectory
        $binaryPath = Join-Path $tempDir "$BinaryName.exe"
        if (-not (Test-Path $binaryPath)) {
            $subDirPath = Join-Path $tempDir "$BinaryName-$target" "$BinaryName.exe"
            if (Test-Path $subDirPath) {
                $binaryPath = $subDirPath
            } else {
                # Find it anywhere in extracted directory
                $foundBinary = Get-ChildItem -Path $tempDir -Recurse -Filter "$BinaryName.exe" | Select-Object -First 1
                if ($foundBinary) {
                    $binaryPath = $foundBinary.FullName
                } else {
                    Write-Error "Could not find $BinaryName.exe in archive"
                    exit 1
                }
            }
        }

        $destPath = Join-Path $InstallDir "$BinaryName.exe"

        # Overwrite existing binary without prompting
        Copy-Item -Path $binaryPath -Destination $destPath -Force

        Write-Host ""
        Write-Host "Successfully installed $BinaryName $version to $destPath"

        # Check if install dir is in PATH and add it automatically
        $currentPath = [Environment]::GetEnvironmentVariable("PATH", "User")
        if ($currentPath -notlike "*$InstallDir*") {
            $newPath = "$InstallDir;$currentPath"
            [Environment]::SetEnvironmentVariable("PATH", $newPath, "User")
            Write-Host ""
            Write-Host "Added $InstallDir to PATH."
            Write-Host "Restart your terminal for changes to take effect."
        }
    }
    finally {
        Remove-Item -Path $tempDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

Install-MailboxMcp
