#!/bin/sh
set -eu

# Alius CLI Installer
# Usage: curl -fsSL https://raw.githubusercontent.com/AliusTech/alius/main/scripts/install/install.sh | sh

REPO="AliusTech/alius"
BINARY_NAME="alius"
TMP_DIR=""

# Color output helpers
info() {
  printf "\033[1;34m%s\033[0m\n" "$1"
}

warn() {
  printf "\033[1;33m%s\033[0m\n" "$1"
}

error() {
  printf "\033[1;31m%s\033[0m\n" "$1" >&2
  exit 1
}

completed() {
  printf "\033[1;32m%s\033[0m\n" "$1"
}

# Check if command exists
has() {
  command -v "$1" >/dev/null 2>&1
}

# Detect platform
detect_platform() {
  local os
  os="$(uname -s)"
  case "$os" in
    Darwin*)  echo "macos" ;;
    Linux*)   echo "linux" ;;
    CYGWIN*|MINGW*|MSYS*) echo "windows" ;;
    *)        error "Unsupported platform: $os" ;;
  esac
}

# Detect architecture
detect_arch() {
  local arch
  arch="$(uname -m)"
  case "$arch" in
    x86_64|amd64)  echo "x64" ;;
    aarch64|arm64)  echo "arm64" ;;
    *)              error "Unsupported architecture: $arch" ;;
  esac
}

# Resolve artifact name
resolve_artifact() {
  local platform="$1"
  local arch="$2"

  case "${platform}-${arch}" in
    linux-x64)      echo "alius-linux-x64.tar.gz" ;;
    macos-x64)      echo "alius-macos-x64.tar.gz" ;;
    macos-arm64)    echo "alius-macos-arm64.tar.gz" ;;
    windows-x64)    echo "alius-windows-x64.zip" ;;
    *)              error "Unsupported platform/arch combination: ${platform}-${arch}" ;;
  esac
}

# Fetch latest version from GitHub API
fetch_latest_version() {
  local url="https://api.github.com/repos/${REPO}/releases/latest"
  local response

  if has curl; then
    response="$(curl -fsSL "$url" 2>/dev/null)" || error "Failed to fetch latest version from GitHub"
  elif has wget; then
    response="$(wget -qO- "$url" 2>/dev/null)" || error "Failed to fetch latest version from GitHub"
  else
    error "Either curl or wget is required"
  fi

  if has jq; then
    echo "$response" | jq -r '.tag_name' | sed 's/^v//'
  else
    echo "$response" | grep '"tag_name"' | head -1 | sed -E 's/.*"tag_name": *"v?([^"]+)".*/\1/'
  fi
}

# Download file
download() {
  local url="$1"
  local dest="$2"

  if has curl; then
    curl -fsSL --progress-bar -o "$dest" "$url" || error "Download failed: $url"
  elif has wget; then
    wget -q --show-progress -O "$dest" "$url" || error "Download failed: $url"
  else
    error "Either curl or wget is required"
  fi
}

# Show usage
usage() {
  cat <<EOF
Alius CLI Installer

Usage:
  install.sh [OPTIONS]

Options:
  -v, --version VERSION    Install specific version (e.g., 0.6.15)
  -b, --bin-dir DIR        Installation directory (default: /usr/local/bin or ~/.local/bin)
  -y, --yes                Skip confirmation prompt
  -h, --help               Show this help message

Environment Variables:
  ALIUS_VERSION            Version to install (alternative to --version)
  ALIUS_INSTALL_DIR        Installation directory (alternative to --bin-dir)

Examples:
  # Install latest version
  curl -fsSL https://raw.githubusercontent.com/AliusTech/alius/main/scripts/install/install.sh | sh

  # Install specific version
  ALIUS_VERSION=0.6.15 sh install.sh

  # Install to custom directory
  sh install.sh --bin-dir ~/.local/bin
EOF
  exit 0
}

# Parse arguments
parse_args() {
  while [ $# -gt 0 ]; do
    case "$1" in
      -v|--version)
        ALIUS_VERSION="$2"
        shift 2
        ;;
      -b|--bin-dir)
        ALIUS_INSTALL_DIR="$2"
        shift 2
        ;;
      -y|--yes)
        SKIP_CONFIRM=true
        shift
        ;;
      -h|--help)
        usage
        ;;
      *)
        error "Unknown option: $1"
        ;;
    esac
  done
}

# Determine install directory
get_install_dir() {
  if [ -n "${ALIUS_INSTALL_DIR:-}" ]; then
    echo "$ALIUS_INSTALL_DIR"
    return
  fi

  if [ -d "/usr/local/bin" ] && [ -w "/usr/local/bin" ]; then
    echo "/usr/local/bin"
  elif [ -d "$HOME/.local/bin" ]; then
    echo "$HOME/.local/bin"
  else
    echo "$HOME/.local/bin"
  fi
}

# Check if directory is in PATH
is_in_path() {
  case ":${PATH}:" in
    *:"$1":*) return 0 ;;
    *)        return 1 ;;
  esac
}

# Main installation
main() {
  parse_args "$@"

  info "Installing Alius CLI..."

  # Detect platform and architecture
  local platform arch artifact version
  platform="$(detect_platform)"
  arch="$(detect_arch)"
  artifact="$(resolve_artifact "$platform" "$arch")"

  info "Platform: ${platform}-${arch}"

  # Determine version
  if [ -n "${ALIUS_VERSION:-}" ]; then
    version="$ALIUS_VERSION"
  else
    info "Fetching latest version..."
    version="$(fetch_latest_version)"
  fi

  info "Version: ${version}"

  # Construct download URL
  local url="https://github.com/${REPO}/releases/download/v${version}/${artifact}"
  info "Download URL: ${url}"

  # Create temp directory
  TMP_DIR="$(mktemp -d)"
  trap 'if [ -n "${TMP_DIR:-}" ]; then rm -rf "$TMP_DIR"; fi' EXIT

  # Download artifact
  info "Downloading ${artifact}..."
  download "$url" "${TMP_DIR}/${artifact}"

  # Extract artifact
  info "Extracting..."
  cd "$TMP_DIR"
  case "$artifact" in
    *.tar.gz)
      tar -xzf "$artifact" || error "Failed to extract tar.gz"
      ;;
    *.zip)
      if has unzip; then
        unzip -o "$artifact" || error "Failed to extract zip"
      else
        error "unzip is required for Windows archives"
      fi
      ;;
  esac

  # Determine install directory
  local bin_dir
  bin_dir="$(get_install_dir)"
  mkdir -p "$bin_dir"

  # Install binary
  info "Installing to ${bin_dir}..."
  if [ "$platform" = "windows" ]; then
    mv "${TMP_DIR}/${BINARY_NAME}.exe" "${bin_dir}/${BINARY_NAME}.exe" || error "Failed to install binary"
    chmod +x "${bin_dir}/${BINARY_NAME}.exe"
  else
    mv "${TMP_DIR}/${BINARY_NAME}" "${bin_dir}/${BINARY_NAME}" || error "Failed to install binary"
    chmod +x "${bin_dir}/${BINARY_NAME}"
  fi

  # Verify installation
  info "Verifying installation..."
  if "${bin_dir}/${BINARY_NAME}" --version >/dev/null 2>&1; then
    local installed_version
    installed_version="$("${bin_dir}/${BINARY_NAME}" --version 2>/dev/null | head -1)"
    completed "Alius CLI installed successfully!"
    info "Version: ${installed_version}"
    info "Location: ${bin_dir}/${BINARY_NAME}"
  else
    warn "Installation completed, but verification failed"
    warn "The binary may not be in your PATH"
  fi

  # Check PATH
  if ! is_in_path "$bin_dir"; then
    warn ""
    warn "WARNING: ${bin_dir} is not in your PATH"
    warn ""
    warn "Add it to your shell profile:"
    warn ""
    if [ -f "$HOME/.zshrc" ]; then
      warn "  echo 'export PATH=\"${bin_dir}:\$PATH\"' >> ~/.zshrc"
    elif [ -f "$HOME/.bashrc" ]; then
      warn "  echo 'export PATH=\"${bin_dir}:\$PATH\"' >> ~/.bashrc"
    else
      warn "  export PATH=\"${bin_dir}:\$PATH\""
    fi
    warn ""
  fi
}

main "$@"
