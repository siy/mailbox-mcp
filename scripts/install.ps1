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

        $binaryPath = Join-Path $tempDir "$BinaryName.exe"
        $destPath = Join-Path $InstallDir "$BinaryName.exe"

        if (Test-Path $destPath) {
            $overwrite = Read-Host "Binary already exists. Overwrite? (y/N)"
            if ($overwrite -ne "y" -and $overwrite -ne "Y") {
                Write-Host "Installation cancelled."
                exit 0
            }
        }

        Copy-Item -Path $binaryPath -Destination $destPath -Force

        Write-Host ""
        Write-Host "Successfully installed $BinaryName $version to $destPath"

        # Check if install dir is in PATH
        $currentPath = [Environment]::GetEnvironmentVariable("PATH", "User")
        if ($currentPath -notlike "*$InstallDir*") {
            Write-Host ""
            Write-Host "Warning: $InstallDir is not in your PATH."
            $addToPath = Read-Host "Add to PATH? (Y/n)"
            if ($addToPath -ne "n" -and $addToPath -ne "N") {
                $newPath = "$InstallDir;$currentPath"
                [Environment]::SetEnvironmentVariable("PATH", $newPath, "User")
                Write-Host "Added to PATH. Restart your terminal for changes to take effect."
            }
        }
    }
    finally {
        Remove-Item -Path $tempDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

Install-MailboxMcp
