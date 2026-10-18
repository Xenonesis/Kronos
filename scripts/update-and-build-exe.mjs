import { execSync } from 'node:child_process';
import { statSync, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '..');

function run(command, cwd = rootDir) {
  console.log(`\n\x1b[36m▶ Running:\x1b[0m ${command} \x1b[90m(in ${path.relative(rootDir, cwd) || '.'})\x1b[0m`);
  execSync(command, { cwd, stdio: 'inherit', shell: true });
}

async function main() {
  console.log('\x1b[1m\x1b[32m=== Paseo Desktop Auto-Pull & .EXE Builder ===\x1b[0m');

  // Step 1: Git Pull
  console.log('\n\x1b[33m[1/5] Pulling latest changes from git...\x1b[0m');
  try {
    run('git pull origin main');
  } catch (err) {
    console.warn('\x1b[33m⚠️ Git pull encountered an issue or working tree has changes, continuing with build...\x1b[0m');
  }

  // Step 2: Build internal packages
  console.log('\n\x1b[33m[2/5] Building core packages & server...\x1b[0m');
  run('npm run build --workspace=@getpaseo/protocol');
  run('npm run build --workspace=@getpaseo/client');
  run('npm run build --workspace=@getpaseo/relay');
  run('npm run build --workspace=@getpaseo/highlight');
  run('npm run build --workspace=@getpaseo/plugin');
  run('npm run build --workspace=@getpaseo/server');

  // Step 3: Export Web Bundle
  console.log('\n\x1b[33m[3/5] Exporting frontend web bundle...\x1b[0m');
  run('npx expo export --platform web', path.join(rootDir, 'packages', 'app'));

  // Step 4: Prepare Server Distribution Assets
  console.log('\n\x1b[33m[4/5] Preparing lean server-dist assets...\x1b[0m');
  run('node prepare-dist.mjs', path.join(rootDir, 'packages', 'desktop-tauri'));

  // Step 5: Compile Tauri NSIS Installer
  console.log('\n\x1b[33m[5/5] Compiling Tauri NSIS installer .exe...\x1b[0m');
  run('npx tauri build --bundles nsis', path.join(rootDir, 'packages', 'desktop-tauri'));

  // Verification
  const exePath = path.join(
    rootDir,
    'packages',
    'desktop-tauri',
    'src-tauri',
    'target',
    'release',
    'bundle',
    'nsis',
    'Paseo_0.12.0_x64-setup.exe'
  );

  if (existsSync(exePath)) {
    const stats = statSync(exePath);
    const sizeMb = (stats.size / (1024 * 1024)).toFixed(2);
    console.log('\n\x1b[1m\x1b[32m✔ SUCCESS! Updated installer created:\x1b[0m');
    console.log(`  \x1b[37mPath:\x1b[0m ${exePath}`);
    console.log(`  \x1b[37mSize:\x1b[0m ${sizeMb} MB (${stats.size.toLocaleString()} bytes)`);
    console.log(`  \x1b[37mLast Modified:\x1b[0m ${stats.mtime.toLocaleString()}`);
  } else {
    console.error('\n\x1b[31m✖ Error: Installer .exe was not found at expected path.\x1b[0m');
    process.exit(1);
  }
}

main().catch((err) => {
  console.error('\n\x1b[31m✖ Build failed:\x1b[0m', err);
  process.exit(1);
});
