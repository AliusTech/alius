#!/usr/bin/env node
/**
 * Platform detection and binary execution wrapper for Alius CLI
 * Automatically selects and runs the correct native binary for the current platform
 *
 * @author Alius Tech
 */

const { spawn } = require('child_process');
const path = require('path');
const fs = require('fs');
const os = require('os');

// Platform detection
const PLATFORM = os.platform();
const ARCH = os.arch();

// Platform to npm package mapping
const PLATFORM_PACKAGES = {
  'darwin-x64': '@alius-tech/alius-darwin-x64',
  'darwin-arm64': '@alius-tech/alius-darwin-arm64',
  'linux-x64': '@alius-tech/alius-linux-x64',
  'linux-arm64': '@alius-tech/alius-linux-arm64',
  'win32-x64': '@alius-tech/alius-win32-x64',
  'win32-arm64': '@alius-tech/alius-win32-arm64',
};

// Binary names per platform
const BINARY_NAMES = {
  darwin: 'alius',
  linux: 'alius',
  win32: 'alius.exe',
};

/**
 * Get the platform key for current system
 */
function getPlatformKey() {
  return `${PLATFORM}-${ARCH}`;
}

/**
 * Get the binary name for current platform
 */
function getBinaryName() {
  return BINARY_NAMES[PLATFORM] || 'alius';
}

/**
 * Find the native binary path
 * Tries multiple locations to handle different installation scenarios
 */
function findBinary() {
  const platformKey = getPlatformKey();
  const packageName = PLATFORM_PACKAGES[platformKey];
  const binaryName = getBinaryName();

  if (!packageName) {
    console.error(`\n✗ Unsupported platform: ${platformKey}`);
    console.error('\nSupported platforms:');
    console.error('  - darwin-x64   (macOS Intel)');
    console.error('  - darwin-arm64 (macOS Apple Silicon)');
    console.error('  - linux-x64    (Linux x86_64)');
    console.error('  - linux-arm64  (Linux ARM64)');
    console.error('  - win32-x64     (Windows x86_64)');
    console.error('  - win32-arm64   (Windows ARM64)');
    process.exit(1);
  }

  // Possible binary locations
  const possiblePaths = [
    // Try platform package's index.js export
    (() => {
      try {
        const platformPkg = require(packageName);
        return platformPkg.binaryPath || platformPkg.getBinaryPath?.();
      } catch { return null; }
    })(),

    // Sibling package directory (npm scoped package and local monorepo layouts)
    path.join(__dirname, '..', '..', packageName.split('/').pop(), 'bin', binaryName),

    // Vendor directory (for bundled distribution)
    path.join(__dirname, '..', 'vendor', platformKey, 'bin', binaryName),

    // Adjacent to this script (development)
    path.join(__dirname, binaryName),
  ].filter(Boolean);

  // Find existing binary
  for (const binaryPath of possiblePaths) {
    if (binaryPath && fs.existsSync(binaryPath)) {
      return binaryPath;
    }
  }

  // Binary not found - show helpful error message
  console.error(`\n✗ Alius binary not found for platform: ${platformKey}`);
  console.error(`\nThis could mean:`);
  console.error('  1. The platform package was not installed correctly');
  console.error('  2. The binary download failed');
  console.error('  3. An incompatible version was installed');
  console.error('\nTroubleshooting:');
  console.error('  1. Clear npm cache: npm cache clean --force');
  console.error('  2. Reinstall: npm uninstall -g @alius-tech/alius && npm install -g @alius-tech/alius');
  console.error('  3. Check platform: node -e "console.log(process.platform + "-" + process.arch)"');
  console.error('\nOr download manually from:');
  console.error('  https://github.com/AliusTech/alius/releases');
  process.exit(1);
}

/**
 * Run the native binary with forwarded arguments and signals
 */
function run() {
  const binaryPath = findBinary();
  const args = process.argv.slice(2);

  // Spawn the native binary
  const child = spawn(binaryPath, args, {
    stdio: 'inherit',
    env: {
      ...process.env,
      // Pass platform info to the binary
      ALIUS_PLATFORM: getPlatformKey(),
      ALIUS_WRAPPER: 'true',
    },
  });

  // Forward termination signals to child process
  const signals = ['SIGINT', 'SIGTERM', 'SIGHUP'];
  signals.forEach((signal) => {
    process.on(signal, () => {
      // Only forward if child is alive
      if (!child.killed) {
        child.kill(signal);
      }
    });
  });

  // Handle child process exit
  child.on('exit', (code, signal) => {
    if (signal) {
      // Re-emit signal so parent exits the same way
      process.kill(process.pid, signal);
    } else {
      process.exit(code ?? 0);
    }
  });

  // Handle child process error
  child.on('error', (err) => {
    console.error(`\n✗ Failed to run Alius: ${err.message}`);
    console.error(`\nBinary path: ${binaryPath}`);
    process.exit(1);
  });
}

// Run CLI
run();
