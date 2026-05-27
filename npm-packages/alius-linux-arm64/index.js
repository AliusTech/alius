// Platform-specific package - exports binary path
const path = require('path');
const binaryName = process.platform === 'win32' ? 'alius.exe' : 'alius';

module.exports = {
  binaryPath: path.join(__dirname, 'bin', binaryName),
};