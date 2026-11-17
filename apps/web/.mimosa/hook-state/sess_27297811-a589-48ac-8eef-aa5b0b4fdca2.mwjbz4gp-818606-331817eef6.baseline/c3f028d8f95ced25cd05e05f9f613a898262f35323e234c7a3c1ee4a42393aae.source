/** Keep Scalar's DOM, CSS, history and event handlers inside its own document. */
export function buildApiDocsDocument(documentUrl: string, scriptUrl: string) {
  // srcdoc is HTML: prevent a URL from terminating the inline script element.
  const options = JSON.stringify({ documentUrl, scriptUrl }).replaceAll(
    '<',
    '\\u003c',
  );
  return `<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>RustSet API</title>
  <style>
    html, body { margin: 0; min-height: 100%; }
    #status { padding: 32px; font: 14px/1.6 system-ui, sans-serif; }
    #status[role="alert"] { color: #b42318; }
  </style>
</head>
<body>
  <div id="status" role="status">正在加载接口文档…</div>
  <div id="app"></div>
  <script>
    const options = ${options};
    const statusElement = document.getElementById('status');
    let reference;
    function showError(message) {
      statusElement.hidden = false;
      statusElement.setAttribute('role', 'alert');
      statusElement.textContent = '接口文档加载失败：' + message + '。请检查网关连接后点击“重新加载”。';
    }
    window.addEventListener('error', (event) => showError(event.message || '文档脚本执行失败'));
    window.addEventListener('unhandledrejection', (event) => showError(event.reason?.message || '文档渲染失败'));
    window.addEventListener('pagehide', () => reference?.destroy());
    async function start() {
      const response = await fetch(options.documentUrl, { signal: AbortSignal.timeout(15000) });
      if (!response.ok) throw new Error('OpenAPI HTTP ' + response.status);
      const content = await response.json();
      if (!content.openapi && !content.swagger) throw new Error('接口未返回有效的 OpenAPI 文档');
      await new Promise((resolve, reject) => {
        const script = document.createElement('script');
        // Vite can add ESM imports to dependency assets during development.
        script.type = 'module';
        const timer = setTimeout(() => reject(new Error('文档脚本加载超时')), 30000);
        script.src = options.scriptUrl;
        script.onload = () => { clearTimeout(timer); resolve(); };
        script.onerror = () => { clearTimeout(timer); reject(new Error('无法加载文档脚本')); };
        document.head.appendChild(script);
      });
      if (!window.Scalar?.createApiReference) throw new Error('文档组件初始化失败');
      reference = window.Scalar.createApiReference('#app', { content });
      statusElement.hidden = true;
    }
    start().catch((error) => showError(error.message));
  </script>
</body>
</html>`;
}
