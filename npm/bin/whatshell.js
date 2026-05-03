#!/usr/bin/env node
const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

const root = path.resolve(__dirname, "../..");
const exe = process.platform === "win32" ? ".exe" : "";
const bundled = path.join(__dirname, `whatshell-${process.platform}-${process.arch}${exe}`);
const target = path.join(root, "target", "release", `whatshell${exe}`);
const bin = fs.existsSync(bundled) ? bundled : target;

if (!fs.existsSync(bin)) {
  console.error("whatshell binary was not built. Run `npm rebuild whatshell` or `cargo build --release`.");
  process.exit(1);
}

const result = spawnSync(bin, process.argv.slice(2), { stdio: "inherit" });
if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}
process.exit(result.status ?? 1);
