/**
 * @alius-tech/alius-linux-x64
 * Platform-specific binary package for Alius CLI
 *
 * @author Alius Tech
 */

const path = require('path');
const fs = require('fs');

const binaryName = 'alius';
const binaryPath = path.join(__dirname, 'bin', binaryName);

/**
 * Get the path to the native binary
 * @returns {string} Absolute path to the binary
 */
function getBinaryPath() {
  if (!fs.existsSync(binaryPath)) {
    throw new Error(
      `Binary not found at ${binaryPath}. \n` +
      `Run 'npm install' to download the binary, or download manually from:\n` +
      `https://github.com/AliusTech/alius/releases`
    );
  }
  return binaryPath;
}

module.exports = {
  binaryPath: binaryPath,
  getBinaryPath: getBinaryPath,
  binaryName: binaryName,
  platform: 'linux-x64',
};