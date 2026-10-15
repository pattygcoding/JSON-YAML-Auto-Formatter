/*
 * JavaScript half of the WebAssembly bridge.
 *
 * The module is built with plain `cargo build --target wasm32-unknown-unknown`, so
 * there is no generated glue to import: this file allocates the two argument
 * strings inside the module's memory, calls `format_input`, and copies the answer
 * back out. The answer buffer belongs to the module and is reused by the next call,
 * which is why it is read immediately.
 *
 * `init` accepts a URL, a Response body, an ArrayBuffer, a typed array or an
 * already compiled WebAssembly.Module, so the browser can fetch and Node can read
 * the file from disk.
 */
(function (global) {
    "use strict";

    const encoder = new TextEncoder();
    const decoder = new TextDecoder("utf-8");
    let instanceExports = null;

    // The site's embedded formatter forces a format by prefixing the input, which is
    // the convention the Go bridge used: a leading marker wins over `mode`.
    const FORCED_MODES = {
        "///force:json///": "json",
        "///force:yaml///": "yaml",
    };

    function splitForcedMode(input, mode) {
        const trimmed = input.trim();
        for (const prefix of Object.keys(FORCED_MODES)) {
            if (trimmed.startsWith(prefix)) {
                return { input: trimmed.slice(prefix.length), mode: FORCED_MODES[prefix] };
            }
        }
        return { input: input, mode: mode };
    }

    function writeString(text) {
        const bytes = encoder.encode(text);
        const pointer = instanceExports.alloc(bytes.length);
        if (bytes.length > 0) {
            new Uint8Array(instanceExports.memory.buffer, pointer, bytes.length).set(bytes);
        }
        return { pointer: pointer, length: bytes.length };
    }

    function readResult(pointer) {
        const view = new DataView(instanceExports.memory.buffer);
        const length = view.getUint32(pointer, true);
        const bytes = new Uint8Array(instanceExports.memory.buffer, pointer + 4, length);
        return decoder.decode(bytes);
    }

    /**
     * Formats `input` as `mode` ("auto", "json" or "yaml") and returns the result as
     * a string. Rejected input comes back as the error message, exactly as the
     * WebAssembly module reported it.
     */
    function format(input, mode) {
        if (instanceExports === null) {
            throw new Error("formatter is not loaded: await FormatterWasm.init() first");
        }
        if (typeof input !== "string") {
            return "No input provided.";
        }
        const request = splitForcedMode(input, typeof mode === "string" ? mode : "auto");
        const inputBuffer = writeString(request.input);
        const modeBuffer = writeString(request.mode);
        let pointer;
        try {
            pointer = instanceExports.format_input(
                inputBuffer.pointer,
                inputBuffer.length,
                modeBuffer.pointer,
                modeBuffer.length
            );
        } finally {
            instanceExports.dealloc(inputBuffer.pointer, inputBuffer.length);
            instanceExports.dealloc(modeBuffer.pointer, modeBuffer.length);
        }
        return readResult(pointer);
    }

    async function toBytes(source) {
        if (typeof source === "string" || (typeof URL !== "undefined" && source instanceof URL)) {
            const response = await fetch(source);
            if (!response.ok) {
                throw new Error("HTTP " + response.status + " fetching " + source);
            }
            return new Uint8Array(await response.arrayBuffer());
        }
        if (source instanceof ArrayBuffer) {
            return new Uint8Array(source);
        }
        if (ArrayBuffer.isView(source)) {
            return new Uint8Array(source.buffer, source.byteOffset, source.byteLength);
        }
        if (typeof WebAssembly.Module === "function" && source instanceof WebAssembly.Module) {
            return source;
        }
        throw new Error("Unsupported formatter source");
    }

    /**
     * Loads the formatter. After this resolves, `formatInput` and `formatJSON` are
     * available on the global object, where the page and the tests look for them.
     */
    async function init(source) {
        const bytes = await toBytes(source);
        const compiled = bytes instanceof WebAssembly.Module ? bytes : await WebAssembly.compile(bytes);
        const instance = await WebAssembly.instantiate(compiled, {});
        instanceExports = instance.exports;
        global.formatInput = format;
        global.formatJSON = format;
        global.FormatterWasm = api;
        return api;
    }

    const api = {
        init: init,
        format: format,
        get loaded() {
            return instanceExports !== null;
        },
    };

    global.FormatterWasm = api;
    if (typeof module === "object" && module !== null && module.exports) {
        module.exports = api;
    }
})(typeof globalThis === "undefined" ? this : globalThis);
