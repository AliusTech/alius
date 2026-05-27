#!/usr/bin/env node
// Script to generate and update all platform-specific npm packages

const fs = require('fs');
const path = require('path');

const VERSION = fs.readFileSync(path.join(__dirname, '..', '.version'), 'utf8').trim();

const PLATFORMS = [
  { name: 'darwin-x64', os: 'darwin', cpu: 'x64', desc: 'macOS Intel (x86_64)' },
  { name: 'darwin-arm64', os: 'darwin', cpu: 'arm64', desc: 'macOS Apple Silicon (ARM64)' },
  { name: 'linux-x64', os: 'linux', cpu: 'x64', desc: 'Linux x86_64' },
  { name: 'linux-arm64', os: 'linux', cpu: 'arm64', desc: 'Linux ARM64' },
  { name: 'win32-x64', os: 'win32', cpu: 'x64', desc: 'Windows x86_64' },
  { name: 'win32-arm64', os: 'win32', cpu: 'arm64', desc: 'Windows ARM64' },
];

const ARTIFACT_MAP = {
  'darwin-x64': 'alius-macos-x64.tar.gz',
  'darwin-arm64': 'alius-macos-arm64.tar.gz',
  'linux-x64': 'alius-linux-x64.tar.gz',
  'linux-arm64': 'alius-linux-arm64.tar.gz',
  'win32-x64': 'alius-windows-x64.zip',
  'win32-arm64': 'alius-windows-arm64.zip',
};

function generatePackageJson(platform) {
  return {
    name: `@aliustech/alius-${platform.name}`,
    version: VERSION,
    description: `Alius CLI binary for ${platform.desc}`,
    author: {
      name: 'Alius Tech',
      email: 'alius@aliustech.com',
      url: 'https://aliustech.com',
    },
    license: 'MIT',
    repository: {
      type: 'git',
      url: 'git+https://github.com/AliusTech/alius.git',
      directory: `npm-packages/alius-${platform.name}`,
    },
    homepage: 'https://github.com/AliusTech/alius#readme',
    main: 'index.js',
    exports: {
      '.': './index.js',
      './binary': './index.js',
    },
    binary: {
      artifact: ARTIFACT_MAP[platform.name],
      downloadUrl: `https://github.com/AliusTech/alius/releases/download/v${VERSION}/${ARTIFACT_MAP[platform.name]}`,
    },
    files: ['bin', 'scripts', 'index.js', 'package.json'],
    scripts: {
      postinstall: 'node scripts/download.js',
    },
    os: [platform.os],
    cpu: [platform.cpu],
    engines: {
      node: '>=16',
    },
  };
}

function generateDownloadScript(platform) {
  const isWindows = platform.name.startsWith('win32');
  const binaryName = isWindows ? 'alius.exe' : 'alius';
  const artifact = ARTIFACT_MAP[platform.name];

  return `#!/usr/bin/env node
/**
 * Download script for @aliustech/alius-${platform.name}
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
const ARTIFACT = pkg.binary.artifact || '${artifact}';
const BINARY_NAME = '${binaryName}';
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
          reject(new Error(\`Download failed: HTTP \${response.statusCode}\`));
          return;
        }

        const totalSize = parseInt(response.headers['content-length'], 10);
        let downloadedSize = 0;

        response.on('data', (chunk) => {
          downloadedSize += chunk.length;
          if (totalSize) {
            const percent = Math.round((downloadedSize / totalSize) * 100);
            if (percent % 10 === 0) {
              console.log(\`Downloading... \${percent}%\`);
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
    execSync(\`tar -xzf "\${archivePath}" -C "\${destDir}"\`, { stdio: 'inherit' });
  } else if (archivePath.endsWith('.zip')) {
    if (process.platform === 'win32') {
      execSync(\`powershell -command "Expand-Archive -Path '\${archivePath}' -DestinationPath '\${destDir}' -Force"\`, { stdio: 'inherit' });
    } else {
      execSync(\`unzip -o "\${archivePath}" -d "\${destDir}"\`, { stdio: 'inherit' });
    }
  }
}

/**
 * Main installation process
 */
async function install() {
  const binDir = path.join(__dirname, '..', 'bin');
  const binaryPath = path.join(binDir, BINARY_NAME);
  const downloadUrl = pkg.binary.downloadUrl || \`https://github.com/\${GITHUB_REPO}/releases/download/v\${VERSION}/\${ARTIFACT}\`;
  const archivePath = path.join(binDir, ARTIFACT);

  // Check if binary already exists
  if (fs.existsSync(binaryPath)) {
    console.log(\`✓ Binary already installed at \${binaryPath}\`);
    console.log(\`  To reinstall, remove the file and run npm install again.\`);
    return;
  }

  // Create bin directory
  if (!fs.existsSync(binDir)) {
    fs.mkdirSync(binDir, { recursive: true });
  }

  console.log(\`\\nInstalling Alius CLI binary...\`);
  console.log(\`Platform: ${platform.desc}\`);
  console.log(\`Version: \${VERSION}\`);
  console.log(\`Artifact: \${ARTIFACT}\\n\`);

  // Download the archive
  try {
    await downloadFile(downloadUrl, archivePath);
    console.log('✓ Download complete');
  } catch (err) {
    console.error(\`✗ Download failed: \${err.message}\`);
    console.error(\`\\nManual download:\`);
    console.error(\`  URL: \${downloadUrl}\`);
    console.error(\`  Extract to: \${binDir}\`);
    process.exit(1);
  }

  // Extract the binary
  try {
    extractArchive(archivePath, binDir);
    console.log('✓ Extraction complete');
  } catch (err) {
    console.error(\`✗ Extraction failed: \${err.message}\`);
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
    console.error(\`✗ Binary not found at \${binaryPath}\`);
    process.exit(1);
  }

  console.log(\`\\n✓ Successfully installed Alius CLI!\`);
  console.log(\`  Binary: \${binaryPath}\\n\`);
}

// Run installation
install().catch((err) => {
  console.error(\`\\n✗ Installation failed: \${err.message}\`);
  console.error(\`\\nFor manual installation, download from:\`);
  console.error(\`  https://github.com/\${GITHUB_REPO}/releases/tag/v\${VERSION}\`);
  process.exit(1);
});`;
}

function generateIndexJs(platform) {
  const isWindows = platform.name.startsWith('win32');
  const binaryName = isWindows ? 'alius.exe' : 'alius';

  return `/**
 * @aliustech/alius-${platform.name}
 * Platform-specific binary package for Alius CLI
 *
 * @author Alius Tech
 */

const path = require('path');
const fs = require('fs');

const binaryName = '${binaryName}';
const binaryPath = path.join(__dirname, 'bin', binaryName);

/**
 * Get the path to the native binary
 * @returns {string} Absolute path to the binary
 */
function getBinaryPath() {
  if (!fs.existsSync(binaryPath)) {
    throw new Error(
      \`Binary not found at \${binaryPath}. \\n\` +
      \`Run 'npm install' to download the binary, or download manually from:\\n\` +
      \`https://github.com/AliusTech/alius/releases\`
    );
  }
  return binaryPath;
}

module.exports = {
  binaryPath: binaryPath,
  getBinaryPath: getBinaryPath,
  binaryName: binaryName,
  platform: '${platform.name}',
};`;
}

// Generate all packages
console.log(`Generating npm packages for version ${VERSION}...\n`);

PLATFORMS.forEach((platform) => {
  const pkgDir = path.join(__dirname, '..', `alius-${platform.name}`);
  const scriptsDir = path.join(pkgDir, 'scripts');
  const binDir = path.join(pkgDir, 'bin');

  // Create directories
  if (!fs.existsSync(scriptsDir)) fs.mkdirSync(scriptsDir, { recursive: true });
  if (!fs.existsSync(binDir)) fs.mkdirSync(binDir, { recursive: true });

  // Write package.json
  fs.writeFileSync(
    path.join(pkgDir, 'package.json'),
    JSON.stringify(generatePackageJson(platform), null, 2) + '\n'
  );

  // Write download script
  fs.writeFileSync(
    path.join(scriptsDir, 'download.js'),
    generateDownloadScript(platform)
  );

  // Write index.js
  fs.writeFileSync(
    path.join(pkgDir, 'index.js'),
    generateIndexJs(platform)
  );

  console.log(`✓ Generated @aliustech/alius-${platform.name}`);
});

// Update main package optionalDependencies
const mainPkgPath = path.join(__dirname, 'alius', 'package.json');
const mainPkg = JSON.parse(fs.readFileSync(mainPkgPath, 'utf8'));
mainPkg.version = VERSION;

// Update optionalDependencies versions
Object.keys(mainPkg.optionalDependencies || {}).forEach((dep) => {
  mainPkg.optionalDependencies[dep] = VERSION;
});

fs.writeFileSync(mainPkgPath, JSON.stringify(mainPkg, null, 2) + '\n');
console.log(`\n✓ Updated main package version to ${VERSION}`);

console.log('\n✓ All packages generated successfully!');