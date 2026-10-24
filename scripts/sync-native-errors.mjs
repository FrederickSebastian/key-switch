import fs from "node:fs";
import vm from "node:vm";
import ts from "typescript";
import assert from "node:assert/strict";

const mappingSource = fs.readFileSync("src/i18n/errors.ts", "utf8");
const mapping = [...mappingSource.matchAll(/  (\w+): "errors\.(\w+)"/g)].map(match => [match[1], match[2]]);
const resources = {};
for (const locale of ["zh-CN", "zh-TW", "en-US", "ja-JP"]) {
  const source = fs.readFileSync(`src/i18n/messages/${locale}.ts`, "utf8");
  const { outputText } = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS } });
  const sandbox = { exports: {} };
  vm.runInNewContext(outputText, sandbox);
  const messages = sandbox.exports.default;
  resources[locale] = Object.fromEntries(mapping.map(([code, key]) => {
    assert.equal(typeof messages.errors[key], "string", `${locale}: ${key}`);
    return [code, messages.errors[key]];
  }));
  Object.assign(resources[locale], messages.statusReasons);
}
const file = "crates/key-switch-core/errors.json";
const serialized = `${JSON.stringify(resources, null, 2)}\n`;
if (process.argv.includes("--check")) {
  assert.equal(fs.readFileSync(file, "utf8").replaceAll("\r\n", "\n"), serialized, "Run node scripts/sync-native-errors.mjs after changing error translations");
  console.log("Native and desktop error translations match");
} else {
  fs.writeFileSync(file, serialized);
}
