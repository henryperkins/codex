import * as esbuild from "esbuild";
import * as fs from "fs";
import * as path from "path";

const OUT_DIR = 'dist'
/**
 * ink attempts to import react-devtools-core in an ESM-unfriendly way:
 *
 * https://github.com/vadimdemedes/ink/blob/eab6ef07d4030606530d58d3d7be8079b4fb93bb/src/reconciler.ts#L22-L45
 *
 * to make this work, we have to strip the import out of the build.
 */
const ignoreReactDevToolsPlugin = {
  name: "ignore-react-devtools",
  setup(build) {
    // When an import for 'react-devtools-core' is encountered,
    // return an empty module.
    build.onResolve({ filter: /^react-devtools-core$/ }, (args) => {
      return { path: args.path, namespace: "ignore-devtools" };
    });
    build.onLoad({ filter: /.*/, namespace: "ignore-devtools" }, () => {
      return { contents: "", loader: "js" };
    });
  },
};

// ---------------------------------------------------------------------------
// Plugin: externalise *everything* that is exactly "jsdom" or starts with it
// ---------------------------------------------------------------------------
const externalizeJsdomPlugin = {
  name: "externalize-jsdom",
  setup(build) {
    // Matches "jsdom" **and** any deep sub-path.
    const filter = /^jsdom($|\/)/;
    build.onResolve({ filter }, args => ({
      path: args.path,
      external: true,
    }));

    // Also externalize canvas which jsdom may try to load
    build.onResolve({ filter: /^canvas($|\/)/ }, args => ({
      path: args.path,
      external: true,
    }));

    // Externalize other potential JSDOM dependencies that might cause issues
    const jsdomDeps = [
      /^parse5($|\/)/,
      /^whatwg-encoding($|\/)/,
      /^whatwg-mimetype($|\/)/,
      /^data-urls($|\/)/,
      /^domexception($|\/)/,
      /^cssstyle($|\/)/,
      /^cssom($|\/)/,
      /^nwsapi($|\/)/,
      /^w3c-hr-time($|\/)/,
      /^w3c-xmlserializer($|\/)/,
      /^xml-name-validator($|\/)/,
      /^html-encoding-sniffer($|\/)/,
      /^tough-cookie($|\/)/,
      /^form-data($|\/)/,
    ];

    jsdomDeps.forEach(depFilter => {
      build.onResolve({ filter: depFilter }, args => ({
        path: args.path,
        external: true,
      }));
    });
  },
};

// ---------------------------------------------------------------------------
// Plugin: externalise everything under "@mozilla/readability"
// ---------------------------------------------------------------------------
const externalizeReadabilityPlugin = {
  name: "externalize-readability",
  setup(build) {
    const filter = /^@mozilla\/readability($|\/)/;
    build.onResolve({ filter }, args => ({
      path: args.path,
      external: true,
    }));

    // Also handle readability without scope
    build.onResolve({ filter: /^readability($|\/)/ }, args => ({
      path: args.path,
      external: true,
    }));
  },
};

// ----------------------------------------------------------------------------
// Build mode detection (production vs development)
//
//  • production (default): minified, external telemetry shebang handling.
//  • development (--dev|NODE_ENV=development|CODEX_DEV=1):
//      – no minification
//      – inline source maps for better stacktraces
//      – shebang tweaked to enable Node's source‑map support at runtime
// ----------------------------------------------------------------------------

const isDevBuild =
  process.argv.includes("--dev") ||
  process.env.CODEX_DEV === "1" ||
  process.env.NODE_ENV === "development";

const plugins = [
  ignoreReactDevToolsPlugin,
  externalizeJsdomPlugin,
  externalizeReadabilityPlugin,
];

// Build Hygiene, ensure we drop previous dist dir and any leftover files
const outPath = path.resolve(OUT_DIR);
if (fs.existsSync(outPath)) {
  fs.rmSync(outPath, { recursive: true, force: true });
}

// Add a shebang that enables source‑map support for dev builds so that stack
// traces point to the original TypeScript lines without requiring callers to
// remember to set NODE_OPTIONS manually.
if (isDevBuild) {
  const devShebangLine =
    "#!/usr/bin/env -S NODE_OPTIONS=--enable-source-maps node\n";
  const devShebangPlugin = {
    name: "dev-shebang",
    setup(build) {
      build.onEnd(async () => {
        const outFile = path.resolve(isDevBuild ? `${OUT_DIR}/cli-dev.js` : `${OUT_DIR}/cli.js`);
        let code = await fs.promises.readFile(outFile, "utf8");
        if (code.startsWith("#!")) {
          code = code.replace(/^#!.*\n/, devShebangLine);
          await fs.promises.writeFile(outFile, code, "utf8");
        }
      });
    },
  };
  plugins.push(devShebangPlugin);
}

esbuild
  .build({
    entryPoints: ["src/cli.tsx"],
    // Do not bundle the contents of package.json at build time: always read it
    // at runtime.
    // Avoid bundling libraries that dynamically require worker scripts (e.g.
    // whatwg-url and jsdom both resolve './xhr-sync-worker.js' at runtime). Bundling
    // breaks the relative path and causes a MODULE_NOT_FOUND error when the CLI
    // is executed. Marking them as external ensures Node can resolve the worker
    // files from their respective directories in node_modules.
    // Mark jsdom and its deep imports as external so worker files like
    // xhr-sync-worker.js stay on disk and resolve correctly at runtime.
    // esbuild patterns can only contain a single "*", so we enumerate up to
    // four directory levels which is enough for jsdom's internal structure.
    external: [
      "../package.json",
      "whatwg-url",
      "canvas",
      // Add more potential JSDOM dependencies
      "parse5",
      "whatwg-encoding",
      "whatwg-mimetype",
      "data-urls",
      "domexception",
      "cssstyle",
      "cssom",
      "nwsapi",
      "w3c-hr-time",
      "w3c-xmlserializer",
      "xml-name-validator",
      "html-encoding-sniffer",
      "tough-cookie",
      "form-data",
      // Explicitly externalize readability
      "@mozilla/readability",
      "readability",
      // leave every jsdom flavour to the plugin
    ],
    bundle: true,
    format: "esm",
    platform: "node",
    tsconfig: "tsconfig.json",
    outfile: isDevBuild ? `${OUT_DIR}/cli-dev.js` : `${OUT_DIR}/cli.js`,
    minify: !isDevBuild,
    sourcemap: isDevBuild ? "inline" : true,
    plugins,
    inject: ["./require-shim.js", "./dirname-shim.js"],
    define: {
      // Define __dirname for direct usage in code
      '__dirname': 'globalThis.__dirname',
      '__filename': 'globalThis.__filename'
    },
  })
  .catch(() => process.exit(1));
