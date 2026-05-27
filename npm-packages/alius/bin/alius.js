#!/usr/bin/env node
// Platform detection and binary execution wrapper
// Similar to @openai/codex implementation

const { spawn } = require('child_process');
const path = require('path');
const fs = require('fs');
const os = require('os');

// Platform to npm package mapping
const PLATFORM_PACKAGES = {
  'darwin-x64': '@aliustech/alius-darwin-x64',
  'darwin-arm64': '@aliustech/alius-darwin-arm64',
  'linux-x64': '@aliustech/alius-linux-x64',
  'linux-arm64': '@aliustech/alius-linux-arm64',
  'win32-x64': '@aliustech/alius-win32-x64',
  'win32-arm64': '@aliustech/alius-win32-arm64',
};

// Rust target triples for each platform
const TARGET_TRIPLES = {
  'darwin-x64': 'x86_64-apple-darwin',
  'darwin-arm64': 'aarch64-apple-darwin',
  'linux-x64': 'x86_64-unknown-linux-musl',
  'linux-arm64': 'aarch64-unknown-linux-musl',
  'win32-x64': 'x86_64-pc-windows-msvc',
  'win32-arm64': 'aarch64-pc-windows-msvc',
};

function getPlatformKey() {
  const platform = os.platform();
  const arch = os.arch();
  return `${platform}-${arch}`;
}

function getBinaryName() {
  return os.platform() === 'win32' ? 'alius.exe' : 'alius';
}

function findBinary() {
  const platformKey = getPlatformKey();
  const packageName = PLATFORM_PACKAGES[platformKey];

  if (!packageName) {
    console.error(`Unsupported platform: ${platformKey}`);
    console.error('Supported platforms: darwin-x64, darwin-arm64, linux-x64, linux-arm64, win32-x64, win32-arm64');
    process.exit(1);
  }

  const binaryName = getBinaryName();

  // Try to find the binary from the platform-specific package
  try {
    // The platform package exports the binary path
    const platformPackage = require(packageName);
    if (platformPackage && platformPackage.binaryPath) {
      return platformPackage.binaryPath;
    }
  } catch (e) {
    // Package might not be installed (npm skips failed optional deps)
  }

  // Try common locations for the binary
  const possiblePaths = [
    // In the platform package's directory
    path.join(__dirname, '..', 'node_modules', packageName, 'bin', binaryName),
    // In platform package with target triple structure
    path.join(__dirname, '..', 'node_modules', packageName, TARGET_TRIPLES[platformKey], 'bin', binaryName),
    // Vendor directory (for bundled distribution)
    path.join(__dirname, '..', 'vendor', TARGET_TRIPLES[platformKey], 'bin', binaryName),
  ];

  for (const binaryPath of possiblePaths) {
    if (fs.existsSync(binaryPath)) {
      return binaryPath;
    }
  }

  // Binary not found
  console.error(`Binary not found for platform: ${platformKey}`);
  console.error('');
  console.error('This could mean:');
  console.error('1. The platform-specific package was not installed');
  console.error('2. The binary download failed');
  console.error('');
  console.error('Try reinstalling:');
  console.error('  npm install -g @aliustech/alius');
  console.error('');
  console.error('Or download directly from:');
  console.error('  https://github.com/AliusTech/alius/releases');
  process.exit(1);
}

function run() {
  const binaryPath = findBinary();

  // Get all args (skip node and script path)
  const args = process.argv.slice(2);

  // Spawn the binary with inherited stdio
  const child = spawn(binaryPath, args, {
    stdio: 'inherit',
    env: process.env,
  });

  // Forward signals to child
  process.on('SIGINT', () => child.kill('SIGINT'));
  process.on('SIGTERM', () => child.kill('SIGTERM'));
  process.on('SIGHUP', () => child.kill('SIGHUP'));

  // Handle child exit
  child.on('exit', (code, signal) => {
    if (signal) {
      process.kill(process.pid, signal);
    } else {
      process.exit(code || 0);
    }
  });

  child.on('error', (err) => {
    console.error(`Failed to spawn binary: ${err.message}`);
    process.exit(1);
  });
}

run();