const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { before, test } = require("node:test");

const formatter = require("../formatter.js");

before(async () => {
    await formatter.init(fs.readFileSync(path.join(__dirname, "..", "formatter.wasm")));
    assert.equal(typeof globalThis.formatInput, "function");
});

for (const [file, format, invalid] of [
    ["json/valid.json", "json", false],
    ["json/invalid.json", "json", true],
    ["yaml/valid.yaml", "yaml", false],
    ["yaml/invalid.yaml", "yaml", true],
    ["yaml/duplicate-keys.yaml", "yaml", true],
    ["yaml/list.yaml", "yaml", false],
    ["yaml/multiple-documents.yaml", "yaml", false],
]) {
    test(`WASM fixture: ${file}`, () => {
        const input = fs.readFileSync(path.join(__dirname, file), "utf8");
        for (const mode of ["auto", format]) {
            const output = globalThis.formatInput(input, mode);
            if (invalid) {
                assert.match(output, new RegExp(`^invalid ${format.toUpperCase()}:`));
            } else {
                assert.doesNotMatch(output, /^invalid /);
                assert.equal(globalThis.formatInput(output, mode), output);
                if (format === "json") {
                    assert.match(output, /\n {4}"name":/);
                    assert.match(output, /9007199254740993/);
                }
            }
        }
    });
}

test("WASM bridge modes and legacy API", () => {
    assert.equal(globalThis.formatJSON, globalThis.formatInput);
    assert.equal(globalThis.formatInput(), "No input provided.");
    assert.equal(globalThis.formatInput(null), "No input provided.");
    assert.equal(globalThis.formatInput("  "), "no input provided");
    assert.match(globalThis.formatInput("name: test", "json"), /^invalid JSON:/);
    assert.equal(globalThis.formatInput("///force:yaml///\n- one\n- two"), "- one\n- two");
    assert.equal(globalThis.formatInput("///force:json///{\"value\":1}"), "{\n    \"value\": 1\n}");
});
