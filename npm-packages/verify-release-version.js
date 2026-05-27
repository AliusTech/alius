#!/usr/bin/env node
const fs = require('fs');
const path = require('path');

const expectedVersion = (process.argv[2] || '').trim().replace(/^v/, '');

if (!expectedVersion) {
  throw new Error('Usage: node npm-packages/verify-release-version.js <version>');
}

const repoRoot = path.join(__dirname, '..');
const packageRoot = __dirname;
const platformPackages = [
  'alius-darwin-x64',
  'alius-darwin-arm64',
  'alius-linux-x64',
  'alius-win32-x64',
];
const expectedOptionalDependencies = platformPackages.map((packageName) => `@alius-tech/${packageName}`).sort();

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, 'utf8'));
}

function assertEqual(label, actual, expected) {
  if (actual !== expected) {
    throw new Error(`${label}: expected ${expected}, got ${actual}`);
  }
}

const fileVersion = fs.readFileSync(path.join(repoRoot, '.version'), 'utf8').trim();
assertEqual('.version', fileVersion, expectedVersion);

const cargoToml = fs.readFileSync(path.join(repoRoot, 'Cargo.toml'), 'utf8');
const cargoVersion = cargoToml.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
assertEqual('Cargo.toml version', cargoVersion, expectedVersion);

const mainPackage = readJson(path.join(packageRoot, 'alius', 'package.json'));
assertEqual(`${mainPackage.name} version`, mainPackage.version, expectedVersion);

const optionalDependencies = Object.keys(mainPackage.optionalDependencies || {}).sort();
assertEqual(`${mainPackage.name} optional dependency list`, optionalDependencies.join(','), expectedOptionalDependencies.join(','));

for (const dependencyName of optionalDependencies) {
  assertEqual(`${mainPackage.name} optional dependency ${dependencyName}`, mainPackage.optionalDependencies[dependencyName], expectedVersion);
}

for (const packageDir of platformPackages) {
  const packageJsonPath = path.join(packageRoot, packageDir, 'package.json');
  const platformPackage = readJson(packageJsonPath);
  assertEqual(`${platformPackage.name} version`, platformPackage.version, expectedVersion);

  const artifact = platformPackage.binary?.artifact;
  const downloadUrl = platformPackage.binary?.downloadUrl;
  const expectedUrl = `https://github.com/AliusTech/alius/releases/download/v${expectedVersion}/${artifact}`;
  assertEqual(`${platformPackage.name} binary.downloadUrl`, downloadUrl, expectedUrl);
}

console.log(`Verified npm package versions match release v${expectedVersion}`);
