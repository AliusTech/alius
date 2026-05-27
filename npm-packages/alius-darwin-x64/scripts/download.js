#!/usr/bin/env node
// Download binary from GitHub Releases

const https = require('https');
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

// Get package info from package.json
const pkg = require('../package.json');
const VERSION = pkg.version;
const PLATFORM = pkg.name.split('-').slice(-2).join('-'); // e.g., 'darwin-arm64'

// Platform to artifact mapping
const ARTIFACTS = {
  'darwin-x64': 'alius-macos-x64.tar.gz',
  'darwin-arm64': 'alius-macos-arm64.tar.gz',
  'linux-x64': 'alius-linux-x64.tar.gz',
  'linux-arm64': 'alius-linux-arm64.tar.gz',
  'win32-x64': 'alius-windows-x64.zip',
  'win32-arm64': 'alius-windows-arm64.zip',
};

const GITHUB_REPO = 'AliusTech/alius';
const ARTIFACT = ARTIFACTS[PLATFORM];
const BINARY_NAME = PLATFORM.startsWith('win32') ? 'alius.exe' : 'alius';

async function download() {
  const binDir = path.join(__dirname, '..', 'bin');
  const downloadUrl = `https://github.com/${GITHUB_REPO}/releases/download/v${VERSION}/${ARTIFACT}`;
  const archivePath = path.join(binDir, ARTIFACT);
  const binaryPath = path.join(binDir, BINARY_NAME);

  // Check if binary already exists
  if (fs.existsSync(binaryPath)) {
    console.log(`Binary already exists at ${binaryPath}`);
    return;
  }

  // Create bin directory
  if (!fs.existsSync(binDir)) {
    fs.mkdirSync(binDir, { recursive: true });
  }

  console.log(`Downloading ${ARTIFACT} from GitHub Releases...`);

  // Download the archive
  await new Promise((resolve, reject) => {
    const file = fs.createWriteStream(archivePath);

    const request = (url) => {
      https.get(url, (response) => {
        if (response.statusCode === 302 || response.statusCode === 301) {
          request(response.headers.location);
          return;
        }

        if (response.statusCode !== 200) {
          reject(new Error(`Download failed: HTTP ${response.statusCode}`));
          return;
        }

        response.pipe(file);

        file.on('finish', () => {
          file.close();
          resolve();
        });
      }).on('error', (err) => {
        fs.unlink(archivePath, () => {});
        reject(err);
      });
    };

    request(downloadUrl);
  });

  console.log('Download complete. Extracting...');

  // Extract the archive
  try {
    if (archivePath.endsWith('.tar.gz')) {
      execSync(`tar -xzf "${archivePath}" -C "${binDir}"`, { stdio: 'inherit' });
    } else if (archivePath.endsWith('.zip')) {
      if (process.platform === 'win32') {
        execSync(`powershell -command "Expand-Archive -Path '${archivePath}' -DestinationPath '${binDir}'"`, { stdio: 'inherit' });
      } else {
        execSync(`unzip -o "${archivePath}" -d "${binDir}"`, { stdio: 'inherit' });
      }
    }
  } catch (err) {
    console.error(`Extraction failed: ${err.message}`);
    process.exit(1);
  }

  // Clean up archive
  fs.unlinkSync(archivePath);

  // Make binary executable (Unix)
  if (process.platform !== 'win32' && fs.existsSync(binaryPath)) {
    fs.chmodSync(binaryPath, 0o755);
  }

  console.log(`Successfully installed binary at ${binaryPath}`);
}

download().catch((err) => {
  console.error(`Failed to download binary: ${err.message}`);
  console.error('');
  console.error('You can manually download from:');
  console.error(`https://github.com/${GITHUB_REPO}/releases/tag/v${VERSION}`);
  console.error('');
  console.error('Then extract the binary to:');
  console.error(`  ${path.join(__dirname, '..', 'bin', BINARY_NAME)}`);
  process.exit(1);
});