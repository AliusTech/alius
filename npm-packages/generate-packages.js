#!/usr/bin/env node
// Script to generate platform-specific npm packages

const fs = require('fs');
const path = require('path');

const PLATFORMS = [
  { name: 'linux-x64', target: 'x86_64-unknown-linux-musl', artifact: 'alius-linux-x64.tar.gz' },
  { name: 'linux-arm64', target: 'aarch64-unknown-linux-musl', artifact: 'alius-linux-arm64.tar.gz' },
  { name: 'darwin-x64', target: 'x86_64-apple-darwin', artifact: 'alius-macos-x64.tar.gz' },
  { name: 'darwin-arm64', target: 'aarch64-apple-darwin', artifact: 'alius-macos-arm64.tar.gz' },
  { name: 'win32-x64', target: 'x86_64-pc-windows-msvc', artifact: 'alius-windows-x64.zip' },
  { name: 'win32-arm64', target: 'aarch64-pc-windows-msvc', artifact: 'alius-windows-arm64.zip' },
];

const VERSION = fs.readFileSync(path.join(__dirname, '..', '.version'), 'utf8').trim();
const GITHUB_REPO = 'AliusTech/alius';

function generatePackageJson(platform) {
  const isWindows = platform.name.startsWith('win32');
  const binaryName = isWindows ? 'alius.exe' : 'alius';

  return {
    name: `@aliustech/alius-${platform.name}`,
    version: VERSION,
    description: `Alius CLI binary for ${platform.name}`,
    author: 'WeiXuG',
    license: 'MIT',
    repository: {
      type: 'git',
      url: 'git+https://github.com/AliusTech/alius.git',
    },
    homepage: 'https://github.com/AliusTech/alius#readme',
    binaryPath: `./bin/${binaryName}`,
    files: ['bin', 'package.json'],
    scripts: {
      postinstall: 'node scripts/download.js',
    },
    os: [platform.name.split('-')[0]],
    cpu: [platform.name.split('-')[1]],
  };
}

function generateDownloadScript(platform) {
  const isWindows = platform.name.startsWith('win32');
  const binaryName = isWindows ? 'alius.exe' : 'alius';

  return `#!/usr/bin/env node
const https = require('https');
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const VERSION = '${VERSION}';
const ARTIFACT = '${platform.artifact}';
const BINARY_NAME = '${binaryName}';
const GITHUB_REPO = '${GITHUB_REPO}';

async function download() {
  const binDir = path.join(__dirname, '..', 'bin');
  const downloadUrl = \`https://github.com/\${GITHUB_REPO}/releases/download/v\${VERSION}/\${ARTIFACT}\`;
  const archivePath = path.join(binDir, ARTIFACT);
  const binaryPath = path.join(binDir, BINARY_NAME);

  if (fs.existsSync(binaryPath)) {
    console.log('Binary already exists, skipping download.');
    return;
  }

  if (!fs.existsSync(binDir)) {
    fs.mkdirSync(binDir, { recursive: true });
  }

  console.log(\`Downloading \${ARTIFACT}...\`);

  await new Promise((resolve, reject) => {
    const file = fs.createWriteStream(archivePath);

    const request = (url) => {
      https.get(url, (res) => {
        if (res.statusCode === 302 || res.statusCode === 301) {
          request(res.headers.location);
          return;
        }
        if (res.statusCode !== 200) {
          reject(new Error(\`Download failed: status \${res.statusCode}\`));
          return;
        }
        res.pipe(file);
        file.on('finish', () => {
          file.close();
          resolve();
        });
      }).on('error', reject);
    };

    request(downloadUrl);
  });

  console.log('Extracting...');

  if (archivePath.endsWith('.tar.gz')) {
    execSync(\`tar -xzf "\${archivePath}" -C "\${binDir}"\`, { stdio: 'inherit' });
  } else if (archivePath.endsWith('.zip')) {
    execSync(\`unzip -o "\${archivePath}" -d "\${binDir}"\`, { stdio: 'inherit' });
  }

  fs.unlinkSync(archivePath);

  if (fs.existsSync(binaryPath)) {
    fs.chmodSync(binaryPath, 0o755);
  }

  console.log('Done!');
}

download().catch(err => {
  console.error(\`Failed to download binary: \${err.message}\`);
  console.error('Please download manually from:');
  console.error(\`https://github.com/\${GITHUB_REPO}/releases/tag/v\${VERSION}\`);
  process.exit(1);
});`;
}

function generateIndexJs(platform) {
  const isWindows = platform.name.startsWith('win32');
  const binaryName = isWindows ? 'alius.exe' : 'alius';

  return `// Platform-specific package for ${platform.name}
module.exports = {
  binaryPath: require('path').join(__dirname, 'bin', '${binaryName}'),
};`;
}

// Generate packages for all platforms
PLATFORMS.forEach(platform => {
  const pkgDir = path.join(__dirname, '..', `alius-${platform.name}`);
  const scriptsDir = path.join(pkgDir, 'scripts');
  const binDir = path.join(pkgDir, 'bin');

  // Create directories
  if (!fs.existsSync(scriptsDir)) fs.mkdirSync(scriptsDir, { recursive: true });
  if (!fs.existsSync(binDir)) fs.mkdirSync(binDir, { recursive: true });

  // Write package.json
  fs.writeFileSync(
    path.join(pkgDir, 'package.json'),
    JSON.stringify(generatePackageJson(platform), null, 2)
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

  console.log(`Generated @aliustech/alius-${platform.name}`);
});

console.log('\\nAll platform packages generated!');