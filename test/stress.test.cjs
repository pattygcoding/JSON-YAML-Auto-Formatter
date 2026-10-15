const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { before, test } = require("node:test");

const formatter = require("../formatter.js");

before(async () => {
    await formatter.init(fs.readFileSync(path.join(__dirname, "..", "formatter.wasm")));
    assert.equal(typeof globalThis.formatInput, "function");
});

const directory = path.join(__dirname, "stress");
const files = fs.readdirSync(directory).filter((file) => /\.(json|yaml)$/.test(file));
assert.equal(files.length, 6, "Expected six generated stress fixtures");

for (const file of files) {
    test(`WASM stress: ${file}`, (context) => {
        const input = fs.readFileSync(path.join(directory, file), "utf8");
        const format = path.extname(file).slice(1);
        for (const mode of ["auto", format]) {
            const started = performance.now();
            const output = globalThis.formatInput(input, mode);
            context.diagnostic(`${mode}: ${input.length} input bytes, ${output.length} output bytes, ${(performance.now() - started).toFixed(1)} ms`);
            if (file.includes("-invalid.")) {
                assert.ok(output.startsWith(`invalid ${format.toUpperCase()}:`), "Expected validation error");
            } else {
                assert.ok(!output.startsWith("invalid "), "Expected valid output");
                assert.ok(output.includes("9007199254740993"), "Large integer must be preserved");
                assert.ok(globalThis.formatInput(output, mode) === output, "Formatting must be stable");
            }
        }
    });
}
