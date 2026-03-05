import type { BunPlugin } from "bun";
import { resolve } from "path";

// Force automerge to use the base64 WASM variant instead of the nodejs variant.
// The nodejs variant uses __dirname + readFileSync, which gets baked as the build
// machine's absolute path and breaks on other machines.
const automergeBase64Plugin: BunPlugin = {
  name: "automerge-base64",
  setup(build) {
    build.onResolve({ filter: /^@automerge\/automerge$/ }, () => ({
      path: resolve(
        import.meta.dirname,
        "node_modules/@automerge/automerge/dist/mjs/entrypoints/fullfat_base64.js"
      ),
    }));

    // Fix deprecated initSync() call in wasm_bindgen output
    build.onLoad(
      { filter: /wasm_bindgen_output\/web\/index\.js$/ },
      async ({ path }) => {
        const text = await Bun.file(path).text();
        return {
          contents: text.replace("initSync(wasmBlob)", "initSync({ module: wasmBlob })"),
          loader: "js",
        };
      }
    );
  },
};

const [target, outfile = "bkey"] = process.argv.slice(2);

const result = await Bun.build({
  entrypoints: ["./cli.ts"],
  compile: {
    ...(target && { target: target as any }),
    outfile,
  },
  plugins: [automergeBase64Plugin],
});

if (!result.success) {
  for (const log of result.logs) console.error(log);
  process.exit(1);
}
