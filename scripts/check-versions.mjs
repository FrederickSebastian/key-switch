import fs from "node:fs";
import assert from "node:assert/strict";

const version = JSON.parse(fs.readFileSync("package.json", "utf8")).version;
const lock = JSON.parse(fs.readFileSync("package-lock.json", "utf8"));
const config = JSON.parse(fs.readFileSync("src-tauri/tauri.conf.json", "utf8"));
assert.equal(lock.version, version);
assert.equal(lock.packages[""].version, version);
assert.equal(config.version, version);
assert.equal(config.bundle.windows.wix.version, version);
for (const file of ["src-tauri/Cargo.toml", "crates/key-switch-core/Cargo.toml", "crates/key-switch-tui/Cargo.toml"]) {
  assert.equal(fs.readFileSync(file, "utf8").match(/^version = "([^"]+)"/m)?.[1], version, file);
}
const cargoLock = fs.readFileSync("Cargo.lock", "utf8");
for (const name of ["key-switch", "key-switch-core", "key-switch-tui"]) {
  assert.equal(cargoLock.match(new RegExp(`name = "${name}"\\r?\\nversion = "([^"]+)"`))?.[1], version, name);
}
if (process.env.RELEASE_TAG) assert.equal(process.env.RELEASE_TAG, `v${version}`);
console.log(`Application, workspace and release versions match: ${version}`);
