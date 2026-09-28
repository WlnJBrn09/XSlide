/**
 * Build the desktop icon.png from static/assets/logo.png.
 */
'use strict';

const fs = require('fs');
const path = require('path');

const root = path.resolve(__dirname, '..');
const src = path.join(root, 'static', 'assets', 'logo.png');
const buildDir = path.join(root, 'build');
const outPng = path.join(buildDir, 'icon.png');

function main() {
  if (!fs.existsSync(src)) {
    console.error('Missing logo:', src);
    process.exit(1);
  }
  fs.mkdirSync(buildDir, { recursive: true });
  fs.copyFileSync(src, outPng);
  console.log('Wrote', outPng, '(' + fs.statSync(outPng).size + ' bytes)');
}

main();
