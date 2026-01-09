#!/bin/sh
# mailbox-mcp installer for Linux and macOS
# Usage: curl -fsSL https://raw.githubusercontent.com/siy/mailbox-mcp/master/scripts/install.sh | sh

set -e

REPO="siy/mailbox-mcp"
BINARY_NAME="mailbox-mcp"
INSTALL_DIR="${HOME}/.local/bin"

# Detect architecture
detect_arch() {
    arch=$(uname -m)
    case "$arch" in
        x86_64|amd64)
            echo "x86_64"
            ;;
        aarch64|arm64)
            echo "aarch64"
            ;;
        *)
            echo "Unsupported architecture: $arch" >&2
            exit 1
            ;;
    esac
}

# Detect OS
detect_os() {
    os=$(uname -s)
    case "$os" in
        Darwin)
            echo "apple-darwin"
            ;;
        Linux)
            echo "unknown-linux-gnu"
            ;;
        *)
            echo "Unsupported OS: $os" >&2
            exit 1
            ;;
    esac
}

# Get latest release tag from GitHub
get_latest_release() {
    if command -v curl >/dev/null 2>&1; then
        curl -sL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/'
    elif command -v wget >/dev/null 2>&1; then
        wget -qO- "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/'
    else
        echo "Error: curl or wget is required" >&2
        exit 1
    fi
}

# Download and install
install() {
    arch=$(detect_arch)
    os=$(detect_os)
    target="${arch}-${os}"

    echo "Detecting system..."
    echo "  Architecture: ${arch}"
    echo "  OS: ${os}"
    echo "  Target: ${target}"

    echo "Fetching latest release..."
    version=$(get_latest_release)
    if [ -z "$version" ]; then
        echo "Error: Could not determine latest version" >&2
        exit 1
    fi
    echo "  Version: ${version}"

    archive_name="${BINARY_NAME}-${target}.tar.gz"
    download_url="https://github.com/${REPO}/releases/download/${version}/${archive_name}"

    echo "Downloading ${archive_name}..."

    tmp_dir=$(mktemp -d)
    trap 'rm -rf "$tmp_dir"' EXIT

    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$download_url" -o "${tmp_dir}/${archive_name}"
    else
        wget -q "$download_url" -O "${tmp_dir}/${archive_name}"
    fi

    echo "Extracting..."
    tar -xzf "${tmp_dir}/${archive_name}" -C "$tmp_dir"

    echo "Installing to ${INSTALL_DIR}..."
    mkdir -p "$INSTALL_DIR"

    # Find binary - may be at root or in subdirectory
    if [ -f "${tmp_dir}/${BINARY_NAME}" ]; then
        mv "${tmp_dir}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"
    elif [ -f "${tmp_dir}/${BINARY_NAME}-${target}/${BINARY_NAME}" ]; then
        mv "${tmp_dir}/${BINARY_NAME}-${target}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"
    else
        # Find it anywhere in extracted directory
        binary_path=$(find "$tmp_dir" -name "$BINARY_NAME" -type f | head -1)
        if [ -z "$binary_path" ]; then
            echo "Error: Could not find ${BINARY_NAME} in archive" >&2
            exit 1
        fi
        mv "$binary_path" "${INSTALL_DIR}/${BINARY_NAME}"
    fi
    chmod +x "${INSTALL_DIR}/${BINARY_NAME}"

    echo ""
    echo "Successfully installed ${BINARY_NAME} ${version} to ${INSTALL_DIR}/${BINARY_NAME}"

    # Check if install dir is in PATH
    case ":$PATH:" in
        *":${INSTALL_DIR}:"*)
            ;;
        *)
            echo ""
            echo "Warning: ${INSTALL_DIR} is not in your PATH."
            echo "Add this to your shell profile:"
            echo ""
            echo "  export PATH=\"\${HOME}/.local/bin:\${PATH}\""
            ;;
    esac
}

install
