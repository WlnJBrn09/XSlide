'use strict';
const { spawn } = require('child_process');
const path = require('path');
const name = require('../package.json').name + '-native' + (process.platform === 'win32' ? '.exe' : '');
const executable = path.join(__dirname, '..', 'native-host', 'target', 'release', name);
const child = spawn(executable, process.argv.slice(2), { stdio: 'inherit' });
child.on('error', error => { console.error(error.message); process.exitCode = 1; });
child.on('exit', (code, signal) => { process.exitCode = code ?? (signal ? 1 : 0); });
