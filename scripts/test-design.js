#!/usr/bin/env node
/**
 * CruxOS design checks for the shipped UI (static/).
 * - WCAG 2.x contrast of the token pairs in static/crux.css, light and dark
 * - #A1ED72 is never used as a text colour, and green fills carry dark content
 * - app windows are opaque: no backdrop blur, no glass material classes
 * - Phosphor glyphs only (no Material Symbols), no looping animations
 */
'use strict';

const fs = require('fs');
const path = require('path');

const root = path.resolve(__dirname, '..');
const staticDir = path.join(root, 'static');
const read = (name) => fs.readFileSync(path.join(staticDir, name), 'utf8');

let passed = 0;
let failed = 0;
function check(ok, message) {
  if (ok) {
    passed++;
    console.log('  PASS  ' + message);
  } else {
    failed++;
    console.error('  FAIL  ' + message);
  }
}

function luminance(hex) {
  const n = hex.replace('#', '');
  const channels = [0, 2, 4].map((i) => parseInt(n.slice(i, i + 2), 16) / 255);
  const [r, g, b] = channels.map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrast(a, b) {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

function tokens(css, selector) {
  const start = css.indexOf(selector + ' {');
  if (start < 0) return {};
  const body = css.slice(start, css.indexOf('\n}', start));
  const out = {};
  for (const m of body.matchAll(/--crux-([a-z-]+):\s*(#[0-9a-fA-F]{6})\b/g)) out[m[1]] = m[2].toLowerCase();
  return out;
}

const crux = read('crux.css');
const light = tokens(crux, ':root');
const dark = { ...light, ...tokens(crux, 'html[data-theme="dark"]') };

console.log('1. Token values match the CruxOS palette');
const expected = {
  light: { bg: '#ffffff', surface: '#f5f5f7', elevated: '#e8e8ed', border: '#d2d2d7', text: '#1d1d1f',
    'text-secondary': '#6e6e73', accent: '#a1ed72', 'on-accent': '#0a0a0b', 'accent-text': '#37770f',
    selected: '#e5fad8', 'selected-surface': '#ddf3d2', destructive: '#d70015' },
  dark: { bg: '#0a0a0b', surface: '#161618', elevated: '#1f1f22', border: '#2c2c30', text: '#f5f5f7',
    'text-secondary': '#a1a1a6', accent: '#a1ed72', 'on-accent': '#0a0a0b', 'accent-text': '#a1ed72',
    selected: '#2b3c22', 'selected-surface': '#35452c', destructive: '#ff453a' },
};
for (const [scheme, values] of Object.entries(expected)) {
  const actual = scheme === 'light' ? light : dark;
  for (const [name, hex] of Object.entries(values)) check(actual[name] === hex, `${scheme} --crux-${name} is ${hex}`);
}

console.log('\n2. Contrast (WCAG 2.x)');
for (const [scheme, t] of [['light', light], ['dark', dark]]) {
  for (const surface of ['bg', 'surface', 'elevated']) {
    check(contrast(t.text, t[surface]) >= 7, `${scheme} primary text on ${surface} ≥ 7:1 (${contrast(t.text, t[surface]).toFixed(1)})`);
    check(contrast(t['accent-text'], t[surface]) >= 4.5, `${scheme} accent text on ${surface} ≥ 4.5:1 (${contrast(t['accent-text'], t[surface]).toFixed(2)})`);
  }
  for (const surface of ['bg', 'surface']) {
    check(contrast(t['text-secondary'], t[surface]) >= 4.5, `${scheme} secondary text on ${surface} ≥ 4.5:1`);
  }
  for (const fill of ['accent', 'accent-hover', 'accent-pressed']) {
    check(contrast(t['on-accent'], t[fill]) >= 7, `${scheme} glyph on ${fill} fill ≥ 7:1`);
  }
  for (const sel of ['selected', 'selected-surface']) {
    check(contrast(t['selected-text'], t[sel]) >= 7, `${scheme} text on ${sel} row ≥ 7:1`);
  }
  check(contrast(t.destructive, t.bg) >= 4.5, `${scheme} destructive on bg ≥ 4.5:1`);
}
check(contrast('#37770f', light.bg) >= 3, 'light focus edge #37770F ≥ 3:1 (non-text rule)');
check(contrast('#a1ed72', dark.surface) >= 3, 'dark focus ring #A1ED72 ≥ 3:1 (non-text rule)');

console.log('\n3. Rules over the shipped stylesheets and markup');
const cssFiles = fs.readdirSync(staticDir).filter((f) => f.endsWith('.css'));
const css = cssFiles.map(read).join('\n');
const markup = fs.readdirSync(staticDir).filter((f) => /\.(html|js)$/.test(f)).map(read).join('\n');
check(!/(^|[^-])color:\s*(var\(--crux-accent\)|#a1ed72)/im.test(css.replace(/html\[data-theme="dark"\][^{]*\{[^}]*\}/g, '')),
  '#A1ED72 is never a text colour on light surfaces');
check(!/backdrop-filter\s*:\s*(?!none)/.test(css), 'no backdrop blur: app windows are opaque');
check(!/liquid-glass|lg-refract|--specular-/.test(css + markup), 'no liquid-glass material or specular driver');
check(!/Material Symbols|material-symbols/.test(css + markup), 'no Material Symbols (UI glyphs are Phosphor)');
check(fs.existsSync(path.join(staticDir, 'assets', 'Phosphor.woff2')), 'Phosphor font is bundled');
// Ligature text (<span class="icon">name</span>) renders unreliably in WebKitGTK; use codepoint classes.
check(!/class="icon[" ]/.test(markup), 'icons use Phosphor codepoint classes, not ligature text');
const phosphor = read(path.join('assets', 'phosphor.css'));
const iconNames = [...markup.matchAll(/\bph ph-([a-z0-9-]+)/g)].map((m) => m[1]);
const unknown = iconNames.filter((name) => !phosphor.includes(`.ph.ph-${name}:before`));
check(iconNames.length > 0 && unknown.length === 0, `every icon exists in Phosphor${unknown.length ? ': ' + unknown.join(', ') : ''}`);
check(!/animation[^;{}]*infinite/.test(css), 'no looping animations');
check(/prefers-reduced-motion/.test(crux), 'reduce motion falls back to fades');
check(/:focus-visible/.test(crux), 'focus glow defined');

console.log(`\npassed=${passed} failed=${failed}`);
process.exitCode = failed ? 1 : 0;
