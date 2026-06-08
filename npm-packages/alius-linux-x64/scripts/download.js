#!/usr/bin/env node
/**
 * Download script for @alius-tech/alius-linux-x64
 * Downloads the native binary from GitHub Releases
 *
 * @author Alius Tech
 */

const https = require('https');
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

// Package configuration
const pkg = require('../package.json');
const VERSION = pkg.version;
const RELEASE_TAG = pkg.binary.releaseTag || '0.6.16';
const ARTIFACT = pkg.binary.artifact || 'alius-linux-x64.tar.gz';
const BINARY_NAME = 'alius';
const GITHUB_REPO = 'AliusTech/alius';

/**
 * Download file from URL with redirect support
 */
async function downloadFile(url, destPath) {
  return new Promise((resolve, reject) => {
    const file = fs.createWriteStream(destPath);

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

        const totalSize = parseInt(response.headers['content-length'], 10);
        let downloadedSize = 0;

        response.on('data', (chunk) => {
          downloadedSize += chunk.length;
          if (totalSize) {
            const percent = Math.round((downloadedSize / totalSize) * 100);
            if (percent % 10 === 0) {
              console.log(`Downloading... ${percent}%`);
            }
          }
        });

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

/**
 * Extract archive to destination directory
 */
function extractArchive(archivePath, destDir) {
  console.log('Extracting binary...');

  if (archivePath.endsWith('.tar.gz')) {
    execSync(`tar -xzf "${archivePath}" -C "${destDir}"`, { stdio: 'inherit' });
  } else if (archivePath.endsWith('.zip')) {
    if (process.platform === 'win32') {
      execSync(`powershell -command "Expand-Archive -Path '${archivePath}' -DestinationPath '${destDir}' -Force"`, { stdio: 'inherit' });
    } else {
      execSync(`unzip -o "${archivePath}" -d "${destDir}"`, { stdio: 'inherit' });
    }
  }
}

/**
 * Main installation process
 */
async function install() {
  const binDir = path.join(__dirname, '..', 'bin');
  const binaryPath = path.join(binDir, BINARY_NAME);
  const downloadUrl = pkg.binary.downloadUrl || `https://github.com/${GITHUB_REPO}/releases/download/${RELEASE_TAG}/${ARTIFACT}`;
  const archivePath = path.join(binDir, ARTIFACT);

  // Check if binary already exists
  if (fs.existsSync(binaryPath)) {
    console.log(`✓ Binary already installed at ${binaryPath}`);
    console.log(`  To reinstall, remove the file and run npm install again.`);
    return;
  }

  // Create bin directory
  if (!fs.existsSync(binDir)) {
    fs.mkdirSync(binDir, { recursive: true });
  }

  console.log(`\nInstalling Alius CLI binary...`);
  console.log(`Platform: Linux x86_64`);
  console.log(`Version: ${VERSION}`);
  console.log(`Artifact: ${ARTIFACT}\n`);

  // Download the archive
  try {
    await downloadFile(downloadUrl, archivePath);
    console.log('✓ Download complete');
  } catch (err) {
    console.error(`✗ Download failed: ${err.message}`);
    console.error(`\nManual download:`);
    console.error(`  URL: ${downloadUrl}`);
    console.error(`  Extract to: ${binDir}`);
    process.exit(1);
  }

  // Extract the binary
  try {
    extractArchive(archivePath, binDir);
    console.log('✓ Extraction complete');
  } catch (err) {
    console.error(`✗ Extraction failed: ${err.message}`);
    process.exit(1);
  }

  // Clean up archive
  fs.unlinkSync(archivePath);

  // Make binary executable (Unix)
  if (process.platform !== 'win32' && fs.existsSync(binaryPath)) {
    fs.chmodSync(binaryPath, 0o755);
  }

  // Verify installation
  if (!fs.existsSync(binaryPath)) {
    console.error(`✗ Binary not found at ${binaryPath}`);
    process.exit(1);
  }

  console.log(`\n✓ Successfully installed Alius CLI!`);
  console.log(`  Binary: ${binaryPath}\n`);
}

// Run installation
install().catch((err) => {
  console.error(`\n✗ Installation failed: ${err.message}`);
  console.error(`\nFor manual installation, download from:`);
  console.error(`  https://github.com/${GITHUB_REPO}/releases/tag/${RELEASE_TAG}`);
  process.exit(1);
});