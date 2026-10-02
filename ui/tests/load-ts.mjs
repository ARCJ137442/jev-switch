import { fileURLToPath } from 'node:url';
import { build } from 'esbuild';

// Bundle test targets so layout modules can share their production dependencies.
export async function loadTs(path) {
  const result = await build({
    entryPoints: [fileURLToPath(new URL(path, import.meta.url))],
    bundle: true,
    format: 'esm',
    platform: 'browser',
    target: 'es2022',
    write: false,
  });
  return import(`data:text/javascript;base64,${Buffer.from(result.outputFiles[0].contents).toString('base64')}`);
}
