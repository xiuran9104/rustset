// Run from apps/web: bun run apps/web-antd/e2e/api-docs.ts
// Real Vue/Scalar/browser; only the host layout and OpenAPI backend are fixtures.
import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';

import vue from '@vitejs/plugin-vue';
import { chromium } from 'playwright';
import { createServer } from 'vite';

const root = fileURLToPath(new URL('../', import.meta.url));
let failDocument = false;
const server = await createServer({
  configFile: false,
  root,
  logLevel: 'error',
  optimizeDeps: { noDiscovery: true, include: ['vue'], entries: [] },
  server: { host: '127.0.0.1', port: 0 },
  plugins: [
    {
      name: 'api-docs-fixture',
      enforce: 'pre',
      resolveId(id) {
        if (['@vben/common-ui', '@vben/hooks'].includes(id)) return `\0${id}`;
      },
      load(id) {
        if (id === '\0@vben/common-ui')
          return `import { h } from 'vue'; export const Page = { setup(_, { slots }) { return () => h('main', slots.default?.()); } };`;
        if (id === '\0@vben/hooks')
          return `export const useAppConfig = () => ({ apiURL: '/api' });`;
      },
      configureServer(vite) {
        vite.middlewares.use('/api/openapi.json', (_req, res) => {
          res.statusCode = failDocument ? 503 : 200;
          res.setHeader('Content-Type', 'application/json');
          res.end(
            JSON.stringify({
              openapi: '3.1.0',
              info: { title: 'Regression API', version: '1.0' },
              paths: {
                '/health': {
                  get: {
                    summary: 'Gateway health',
                    responses: { 200: { description: 'Healthy' } },
                  },
                },
              },
            }),
          );
        });
        vite.middlewares.use('/__api-docs-test', async (_req, res) => {
          res.setHeader('Content-Type', 'text/html');
          res.end(
            await vite.transformIndexHtml(
              '/__api-docs-test',
              `<!doctype html><html><head><title>Host application</title></head><body><div id="host"></div><script type="module">
            import { createApp, h, KeepAlive, ref } from 'vue';
            import Docs from '/src/views/infra/api-docs/index.vue';
            const Other = { name: 'OtherPage', setup: () => () => h('h1', { id: 'other' }, 'Other page') };
            createApp({ setup() { const docs = ref(true); return () => h('div', [
              h('button', { id: 'toggle', onClick: () => docs.value = !docs.value }, 'Switch page'),
              h(KeepAlive, null, { default: () => h(docs.value ? Docs : Other) })
            ]); } }).mount('#host');
          </script></body></html>`,
            ),
          );
        });
      },
    },
    vue(),
  ],
});

await server.listen();
const browser = await chromium.launch({
  channel: process.env.PLAYWRIGHT_CHANNEL,
});
try {
  const page = await browser.newPage();
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto(`${server.resolvedUrls!.local[0]}__api-docs-test`);
  const reference = () => page.frameLocator('iframe');
  for (let index = 0; index < 3; index++) {
    await reference()
      .getByRole('heading', { name: 'Regression API', exact: true })
      .waitFor({ timeout: 60_000 });
    assert.equal(await page.title(), 'Host application');
    assert.equal(
      await page.locator('#scalar-style').count(),
      0,
      'Scalar styles stay inside the frame',
    );
    await page.locator('#toggle').click();
    await page.locator('#other').waitFor();
    assert.equal(
      await page.locator('iframe').count(),
      0,
      'deactivation removes the reference',
    );
    await page.locator('#toggle').click();
  }
  await reference()
    .getByRole('heading', { name: 'Regression API', exact: true })
    .waitFor();
  failDocument = true;
  await page.getByRole('button', { name: '重新加载' }).click();
  await reference().getByRole('alert').filter({ hasText: '503' }).waitFor();
  await page.locator('#toggle').click();
  await page.locator('#other').waitFor();
  failDocument = false;
  await page.locator('#toggle').click();
  await reference()
    .getByRole('heading', { name: 'Regression API', exact: true })
    .waitFor();
  assert.deepEqual(errors, []);
  console.log(
    'PASS: render, repeated KeepAlive navigation, failure feedback and recovery',
  );
} finally {
  await browser.close();
  await server.close();
}
