import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const clientDir = path.resolve(__dirname, '../dist/client');

if (fs.existsSync(clientDir)) {
  // 1. Create .nojekyll so GitHub Pages does not ignore underscore folders
  fs.writeFileSync(path.join(clientDir, '.nojekyll'), '');
  console.log('Created dist/client/.nojekyll');

  // 2. Create 404.html fallback for client-side navigation
  const indexHtml = path.join(clientDir, 'index.html');
  if (fs.existsSync(indexHtml)) {
    fs.copyFileSync(indexHtml, path.join(clientDir, '404.html'));
    console.log('Created dist/client/404.html from index.html');
  }
}
