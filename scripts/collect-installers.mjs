import { readdirSync, mkdirSync, copyFileSync, rmSync, existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const bundleDir = path.join(root, "src-tauri", "target", "release", "bundle");
const outDir = path.join(root, "installers");

const sourceDirs = [path.join(bundleDir, "nsis"), path.join(bundleDir, "msi")];

if (existsSync(outDir)) {
  rmSync(outDir, { recursive: true, force: true });
}
mkdirSync(outDir, { recursive: true });

let count = 0;
for (const dir of sourceDirs) {
  if (!existsSync(dir)) continue;
  for (const file of readdirSync(dir)) {
    if (file.endsWith(".exe") || file.endsWith(".msi")) {
      copyFileSync(path.join(dir, file), path.join(outDir, file));
      console.log(`Copied ${file}`);
      count++;
    }
  }
}

if (count === 0) {
  console.error(
    "No installer files found under src-tauri/target/release/bundle. Did `tauri build` run first?",
  );
  process.exit(1);
}

console.log(`\n${count} installer(s) in ${outDir}`);
