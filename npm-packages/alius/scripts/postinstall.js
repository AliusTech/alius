#!/usr/bin/env node

const https = require('https');
const fs = require('fs');
const path = require('path');
const os = require('os');
const { execSync } = require('child_process');

// Read version from .version file in the npm package
function getVersion() {
  const versionPath = path.join(__dirname, '..', '.version');
  if (fs.existsSync(versionPath)) {
    return fs.readFileSync(versionPath, 'utf8').trim();
  }
  // Fallback to package.json version
  const pkg = require('../package.json');
  return pkg.version;
}

const PACKAGE_VERSION = getVersion();
const GITHUB_REPO = 'AliusTech/alius';

// Determine platform and architecture
function getPlatformInfo() {
  const platform = os.platform();
  const arch = os.arch();

  let target;
  let artifactName;
  let binaryName = 'alius';

  switch (platform) {
    case 'darwin':
      target = arch === 'arm64' ? 'macos-arm64' : 'macos-x64';
      artifactName = `alius-${target}.tar.gz`;
      break;
    case 'linux':
      target = 'linux-x64';
      artifactName = `alius-${target}.tar.gz`;
      break;
    case 'win32':
      target = 'windows-x64';
      artifactName = `alius-${target}.zip`;
      binaryName = 'alius.exe';
      break;
    default:
      throw new Error(`Unsupported platform: ${platform}`);
  }

  return { platform, arch, target, artifactName, binaryName };
}

// Download file from URL
function downloadFile(url, destPath) {
  console.log(`Downloading from: ${url}`);

  return new Promise((resolve, reject) => {
    const file = fs.createWriteStream(destPath);

    const request = (url) => {
      https.get(url, (response) => {
        if (response.statusCode === 302 || response.statusCode === 301) {
          // Follow redirect
          request(response.headers.location);
          return;
        }

        if (response.statusCode !== 200) {
          reject(new Error(`Download failed with status ${response.statusCode}`));
          return;
        }

        response.pipe(file);

        file.on('finish', () => {
          file.close();
          resolve();
        });
      }).on('error', (err) => {
        fs.unlink(destPath, () => {});
        reject(err);
      });
    };

    request(url);
  });
}

// Extract archive
function extractArchive(archivePath, destDir, binaryName) {
  console.log(`Extracting ${archivePath}...`);

  if (archivePath.endsWith('.tar.gz')) {
    // Use tar for .tar.gz files
    execSync(`tar -xzf "${archivePath}" -C "${destDir}"`, { stdio: 'inherit' });
  } else if (archivePath.endsWith('.zip')) {
    // On Windows, we can use PowerShell or a zip utility
    if (os.platform() === 'win32') {
      execSync(`powershell -command "Expand-Archive -Path '${archivePath}' -DestinationPath '${destDir}'"`, { stdio: 'inherit' });
    } else {
      execSync(`unzip -o "${archivePath}" -d "${destDir}"`, { stdio: 'inherit' });
    }
  }

  // Find and move the binary to the correct location
  const extractedBinary = path.join(destDir, binaryName);
  const finalBinary = path.join(destDir, 'alius');

  if (fs.existsSync(extractedBinary) && extractedBinary !== finalBinary) {
    fs.renameSync(extractedBinary, finalBinary);
  }

  // Make binary executable on Unix systems
  if (os.platform() !== 'win32') {
    fs.chmodSync(finalBinary, 0o755);
  }
}

// Main installation process
async function install() {
  console.log('Installing Alius CLI...');

  const { target, artifactName, binaryName } = getPlatformInfo();

  const binDir = path.join(__dirname, '..', 'bin');
  const downloadUrl = `https://github.com/${GITHUB_REPO}/releases/download/v${PACKAGE_VERSION}/${artifactName}`;
  const archivePath = path.join(binDir, artifactName);

  // Ensure bin directory exists
  if (!fs.existsSync(binDir)) {
    fs.mkdirSync(binDir, { recursive: true });
  }

  // Download the archive
  try {
    await downloadFile(downloadUrl, archivePath);
  } catch (err) {
    console.error(`Failed to download binary: ${err.message}`);
    console.error('You may need to download manually from:');
    console.error(`https://github.com/${GITHUB_REPO}/releases/tag/v${PACKAGE_VERSION}`);
    process.exit(1);
  }

  // Extract the binary
  try {
    extractArchive(archivePath, binDir, binaryName);
  } catch (err) {
    console.error(`Failed to extract binary: ${err.message}`);
    process.exit(1);
  }

  // Clean up the archive
  fs.unlinkSync(archivePath);

  console.log('Alius CLI installed successfully!');
  console.log('Run `alius --help` to get started.');
}

install().catch((err) => {
  console.error('Installation failed:', err);
  process.exit(1);
});