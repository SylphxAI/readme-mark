import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import { JSDOM } from 'jsdom';
import { micromark } from 'micromark';
import { gfm, gfmHtml } from 'micromark-extension-gfm';

const base = 'https://mark.sylphx.com';
const template = await readFile(new URL('../templates/index.html', import.meta.url), 'utf8');
const html = template.replaceAll('{{BASE}}', base).replace('{{BOOT}}', 'null');
const script = html.match(/<script>\s*([\s\S]*?)<\/script>/)[1];
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));

// Execute the real studio script and its event handlers, not a second serializer.
// Only network, image decoding, clipboard and the file save are browser fakes.
async function studio(url = base, options = {}) {
  const dom = new JSDOM(html, { url, runScripts: 'outside-only' });
  const { window: w } = dom;
  const copied = [], saved = [], probes = [], requests = [];
  let render = null;
  if (options.controlled) {
    const setTimeout = w.setTimeout.bind(w), clearTimeout = w.clearTimeout.bind(w);
    w.setTimeout = (fn, ms) => ms === 220 ? (render = fn, 'render') : setTimeout(fn, ms);
    w.clearTimeout = id => id === 'render' ? (render = null) : clearTimeout(id);
  }
  const svg = '<svg xmlns="http://www.w3.org/2000/svg" width="880" height="220"/>';
  w.fetch = async url => String(url).includes('catalog')
    ? { ok: true, json: async () => ({}) }
    : (requests.push(String(url)), options.fetchSvg ? options.fetchSvg(String(url))
      : { ok: true, headers: new Map([['content-type', 'image/svg+xml']]), blob: async () => new Blob([svg]) });
  w.Image = class {
    naturalWidth = 880;
    naturalHeight = 220;
    set src(value) {
      this.url = value;
      probes.push(this);
      if (!options.controlled) queueMicrotask(() => this.onload());
    }
  };
  Object.defineProperty(w.navigator, 'clipboard', { value: { writeText: async s => copied.push(s) } });
  w.URL.createObjectURL = blob => { saved.push({ blob }); return 'blob:studio-test'; };
  w.URL.revokeObjectURL = () => {};
  w.HTMLAnchorElement.prototype.click = function () { Object.assign(saved.at(-1), { name: this.download, href: this.href }); };
  w.eval(script);
  await pause(0);
  const click = selector => {
    const button = w.document.querySelector(selector);
    assert.ok(button, selector);
    button.click();
  };
  const field = (key, value) => {
    const label = [...w.document.querySelectorAll('.field em')].find(n => n.textContent === key);
    assert.ok(label, key);
    const input = label.closest('.field').querySelector('input,textarea');
    input.value = value;
    input.dispatchEvent(new w.Event('input', { bubbles: true }));
  };
  const copy = async (format, compact = false) => {
    click(`#fmtSeg [data-fmt="${format}"]`);
    const displayed = w.document.getElementById('codeBox').textContent;
    click(compact ? (format === 'url' ? '#dockUrl' : '#dockMd') : '#copyBtn');
    await pause(0);
    assert.equal(copied.at(-1), displayed, 'copied and displayed output agree');
    return copied.at(-1);
  };
  const kind = name => click(`#kinds button:nth-child(${['banner', 'badge', 'typing', 'icons', 'stats', 'profile', 'score', 'deploy'].indexOf(name) + 1})`);
  const flushRender = () => { assert.ok(render, 'debounced render scheduled'); const fn = render; render = null; fn(); };
  return { w, dom, field, kind, click, copy, saved, probes, requests, flushRender };
}

function assertImage(source, format, alt, url) {
  const fragment = JSDOM.fragment(format === 'md' ? micromark(source, { extensions: [gfm()], htmlExtensions: [gfmHtml()] }) : source);
  assert.equal([...fragment.childNodes].filter(n => n.nodeType !== 3 || n.textContent.trim()).length, 1, source);
  const root = format === 'md' ? fragment.querySelector('p') : fragment;
  assert.ok(root, source);
  assert.equal(root.childNodes.length, 1, source);
  assert.equal(root.firstChild.tagName, 'IMG', source);
  assert.equal(fragment.querySelectorAll('*').length, format === 'md' ? 2 : 1, source);
  const img = root.firstChild;
  assert.equal(img.getAttribute('alt'), alt, source);
  assert.equal(img.getAttribute('src'), url, source);
  assert.deepEqual(img.getAttributeNames().sort(), ['alt', 'src'], source);
  if (format === 'md') {
    const table = JSDOM.fragment(micromark(`| Image |\n| --- |\n| ${source} |`, { extensions: [gfm()], htmlExtensions: [gfmHtml()] }));
    const cell = table.querySelector('tbody td');
    assert.equal(cell.childNodes.length, 1, source);
    assert.equal(cell.firstChild.tagName, 'IMG', source);
    assert.equal(cell.firstChild.getAttribute('alt'), alt, source);
    assert.equal(cell.firstChild.getAttribute('src'), url, source);
  }
}

test('blocked clipboard writes select the requested embed format without claiming success, then recover', async () => {
  for (const [selector, format] of [['#copyBtn', 'html'], ['#dockMd', 'md'], ['#dockUrl', 'url']]) {
    for (const fallback of [() => false, () => { throw new Error('copy denied'); }]) {
      const s = await studio();
      try {
        s.w.HTMLElement.prototype.scrollIntoView = () => {};
        s.click('#fmtSeg [data-fmt="html"]');
        s.click(selector); await pause(0);
        s.w.navigator.clipboard.writeText = async () => { throw new Error('clipboard denied'); };
        s.w.document.execCommand = fallback;
        s.click(selector); await pause(0);
        const code = s.w.document.getElementById('codeBox');
        assert.equal(s.w.document.querySelector('#fmtSeg .on').dataset.fmt, format);
        assert.equal(s.w.getSelection().toString(), code.textContent);
        assert.equal(s.w.document.activeElement, code);
        assert.match(s.w.document.getElementById('toast').textContent, /Copy was blocked.*manually/);
        assert.equal(s.w.document.querySelector(selector).classList.contains('copied'), false);
        assert.equal(s.w.document.querySelectorAll('body > textarea').length, 0);
        s.w.navigator.clipboard.writeText = async () => {};
        s.click(selector); await pause(0);
        assert.match(s.w.document.getElementById('toast').textContent, /copied/);
      } finally { s.dom.window.close(); }
    }
  }
});

test('legacy clipboard fallback reports success only when the browser confirms it', async () => {
  const s = await studio();
  try {
    let fallbackText;
    s.w.navigator.clipboard.writeText = async () => { throw new Error('clipboard unavailable'); };
    s.w.document.execCommand = command => {
      assert.equal(command, 'copy');
      fallbackText = s.w.document.querySelector('body > textarea').value;
      return true;
    };
    const expected = s.w.document.getElementById('codeBox').textContent;
    s.click('#copyBtn'); await pause(0);
    assert.equal(fallbackText, expected);
    assert.equal(s.w.document.getElementById('copyLabel').textContent, 'Copied');
    assert.match(s.w.document.getElementById('toast').textContent, /copied/);
  } finally { s.dom.window.close(); }
});

const cases = [
  'Hello, README',
  'README \\',
  'README \\\\',
  'README [API]',
  '*stars* _under_ `code`',
  '~~strike~~ ![nested](https://example.com)',
  'literal &amp; and &#34;',
  'README [API] & literal &#34;quote&#34;',
  'Hello &amp; goodbye',
  '<angle> "double" \'single\' \\backslash',
  '"><script>alert(1)</script><img onerror="x',
  '&quot; onerror=&quot;alert(1)',
  '](https://example.com) ![extra](/image)',
  'README ) (unbalanced',
  'Unicode 世界 — café 😀 [API] \\',
  'First [line] &amp; \\\nSecond line',
  'x'.repeat(59) + '\\' + 'discarded after the bound',
  ...Array.from('!"#$%&\'()*+,-./:;<=>?@[\\]^_`{|}~', c => `a${c}${c}b${c}`),
];

for (const [kind, key, prefix] of [
  ['banner', 'text', ''],
  ['badge', 'message', 'build: '],
  ['typing', 'lines', ''],
  ['profile', 'text', ''],
]) {
  test(`${kind}: GFM and HTML copied through studio controls preserve literal accessible text`, async () => {
    const s = await studio();
    try {
      s.kind(kind);
      for (const text of cases) {
        // Single-line HTML inputs remove newlines before the input event.
        const entered = ['badge', 'profile'].includes(kind) ? text.replaceAll('\n', '') : text;
        s.field(key, entered);
        const alt = (prefix + entered).split('\n')[0].slice(0, 60);
        const url = await s.copy('url');
        for (const fmt of ['md', 'html']) assertImage(await s.copy(fmt), fmt, alt, url);
        assert.equal(await s.copy('md', true), await s.copy('md'), 'compact Markdown agrees');
      }
    } finally { s.dom.window.close(); }
  });
}

test('all input-derived alt consumers keep first-line/60-character policy and working adjacent controls', async () => {
  const s = await studio();
  try {
    for (const [kind, key, text, alt] of [
      ['badge', 'label', '[build] &amp; \\', '[build] &amp; \\: passing'],
      ['badge', 'message', 'README ) [API] \\', 'build: README ) [API] \\'],
      ['score', 'label', '[score] &amp; \\', '[score] &amp; \\: 92'],
      ['icons', 'i', 'rust,ts', 'rust,ts'],
      ['stats', 'username', 'octocat', 'octocat'],
      ['profile', 'text', 'Ada [API] &amp; \\', 'Ada [API] &amp; \\'],
      ['banner', 'text', 'README [API] \\\nSecond', 'README [API] \\'],
    ]) {
      s.kind(kind); s.field(key, text);
      await pause(250);
      const preview = s.w.document.getElementById('previewImg');
      const url = await s.copy('url');
      assert.equal(preview.src, url);
      assert.equal(preview.alt, alt);
      assert.equal(await s.copy('url', true), url);
      for (const fmt of ['md', 'html']) assertImage(await s.copy(fmt), fmt, alt, url);
      s.click('#downloadBtn'); await pause(0);
      assert.equal(s.saved.at(-1).name, `mark-${kind}.svg`);
      assert.match(await s.saved.at(-1).blob.text(), /^<svg/);
      const hash = s.w.location.hash;
      s.w.document.getElementById('importBox').value = url;
      s.w.document.getElementById('importForm').dispatchEvent(new s.w.Event('submit', { cancelable: true }));
      assert.equal(await s.copy('url'), url, 'URL import retains state');
      assert.equal(s.w.location.hash, hash);
      const markdown = await s.copy('md');
      assertImage(markdown, 'md', alt, url);
      s.w.document.getElementById('importBox').value = markdown;
      s.w.document.getElementById('importForm').dispatchEvent(new s.w.Event('submit', { cancelable: true }));
      assert.equal(await s.copy('url'), url, 'copied Markdown imports escaped brackets and destination parentheses');
      assertImage(await s.copy('html'), 'html', alt, url);
    }
  } finally { s.dom.window.close(); }
});

test('normal title export is unchanged; hash-imported state roundtrips without entity decoding', async () => {
  const path = '/api/v1/mark/hero.svg?text=Hello%2C+README&theme=dark';
  const s = await studio(base + '/#' + path);
  try {
    const url = await s.copy('url');
    assert.equal(await s.copy('md'), `![Hello, README](${url.replaceAll('&', '\\&')})`);
    assert.equal(await s.copy('html'), `<img src="${url.replaceAll('&', '&amp;')}" alt="Hello, README" />`);
    s.field('text', '[API] &amp; &#34; \\');
    const updated = await s.copy('url');
    const imported = await studio(base + '/#' + updated.slice(base.length));
    try {
      assert.equal(await imported.copy('url'), updated);
      for (const fmt of ['md', 'html']) assertImage(await imported.copy(fmt), fmt, '[API] &amp; &#34; \\', updated);
    } finally { imported.dom.window.close(); }
  } finally { s.dom.window.close(); }
});

test('imported entity-shaped query keys keep the displayed image URL in both formats', async () => {
  const s = await studio();
  try {
    const url = base + '/api/v1/mark/hero.svg?text=Imported&amp;=keep';
    s.w.document.getElementById('importBox').value = url;
    s.w.document.getElementById('importForm').dispatchEvent(new s.w.Event('submit', { cancelable: true }));
    const displayed = await s.copy('url');
    assert.ok(displayed.endsWith('&amp;=keep'));
    for (const fmt of ['md', 'html']) assertImage(await s.copy(fmt), fmt, 'Imported', displayed);
    s.w.document.getElementById('importBox').value = await s.copy('md');
    s.w.document.getElementById('importForm').dispatchEvent(new s.w.Event('submit', { cancelable: true }));
    assert.equal(await s.copy('url'), displayed);
  } finally { s.dom.window.close(); }
});

test('malformed badge URLs show recovery feedback without replacing the draft or breaking later imports', async () => {
  const s = await studio();
  try {
    s.field('text', 'Keep my draft');
    const draft = await s.copy('url');
    const errors = [];
    s.w.addEventListener('error', e => errors.push(e.message));
    for (const path of ['build-100%-green', 'build-%FF-green', 'build-%E0%A4-green']) {
      s.w.document.getElementById('importBox').value = 'https://img.shields.io/badge/' + path;
      s.w.document.getElementById('importForm').dispatchEvent(new s.w.Event('submit', { cancelable: true }));
      assert.match(s.w.document.getElementById('importNote').textContent, /try a badge, banner, icons, typing or stats URL/);
      assert.equal(await s.copy('url'), draft);
    }
    assert.deepEqual(errors, []);
    s.w.document.getElementById('importBox').value = 'https://img.shields.io/badge/build-100%25-green';
    s.w.document.getElementById('importForm').dispatchEvent(new s.w.Event('submit', { cancelable: true }));
    assert.match(s.w.document.getElementById('importNote').textContent, /Imported from shields.io/);
    assertImage(await s.copy('md'), 'md', 'build: 100%', await s.copy('url'));
  } finally { s.dom.window.close(); }
});

test('typing literal semicolons and custom separators survive render URLs, imports and exports', async () => {
  const s = await studio();
  try {
    s.kind('typing');
    s.field('lines', ' \n ');
    assert.equal(new URL(await s.copy('url')).searchParams.get('lines'), '');
    for (const text of ['literal &amp; and &#34;\nSecond', 'semi; pipe| caret^ ␞ ␟\nEnd;', 'one;;\ntwo|']) {
      s.field('lines', text);
      const url = await s.copy('url');
      const query = new URL(url).searchParams;
      assert.deepEqual(query.get('lines').split(query.get('separator') || ';'), text.split('\n'));
      for (const fmt of ['md', 'html']) assertImage(await s.copy(fmt), fmt, text.split('\n')[0], url);
      s.w.document.getElementById('importBox').value = url;
      s.w.document.getElementById('importForm').dispatchEvent(new s.w.Event('submit', { cancelable: true }));
      assert.equal(await s.copy('url'), url);
      assertImage(await s.copy('md'), 'md', text.split('\n')[0], url);
    }
    for (const path of ['/typing.svg', '/api/v1/mark/typing.svg']) {
      const url = base + path + '?lines=literal%20%26amp%3B|Second&separator=%7C';
      s.w.document.getElementById('importBox').value = url;
      s.w.document.getElementById('importForm').dispatchEvent(new s.w.Event('submit', { cancelable: true }));
      const displayed = await s.copy('url');
      const query = new URL(displayed).searchParams;
      assert.deepEqual(query.get('lines').split(query.get('separator')), ['literal &amp;', 'Second']);
      assertImage(await s.copy('md'), 'md', 'literal &amp;', displayed);
    }
  } finally { s.dom.window.close(); }
});

const deferred = () => {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
};
const svgResponse = (url, body) => ({
  ok: true,
  headers: new Map([['content-type', 'image/svg+xml']]),
  blob: async () => body ? await body : new Blob([
    `<svg xmlns="http://www.w3.org/2000/svg"><text>${new URL(url).searchParams.get('text')}</text></svg>`,
  ]),
});
function assertPending(s, status = 'loading') {
  assert.equal(s.w.document.getElementById('downloadBtn').disabled, true);
  assert.equal(s.w.document.getElementById('previewStatus').dataset.state, status);
}

test('immediate edit/download invalidates readiness before debounce and saves only the latest SVG bytes', async () => {
  const s = await studio(base, { controlled: true, fetchSvg: url => svgResponse(url) });
  try {
    s.probes.at(-1).onload();
    assert.equal(s.w.document.getElementById('downloadBtn').disabled, false);
    const title = 'Mark export freshness witness 2026-10-29';
    s.field('text', title);
    s.click('#downloadBtn'); // same synchronous turn, no timer or probe completion
    assertPending(s);
    assert.equal(s.requests.length, 0);
    assert.equal(s.saved.length, 0);
    assert.ok(s.w.document.getElementById('codeBox').textContent.includes('Mark+export+freshness'));
    s.flushRender();
    s.probes.at(-1).onload();
    s.click('#downloadBtn');
    await pause(0);
    assert.equal(s.saved.length, 1);
    const bytes = await s.saved[0].blob.text();
    assert.ok(bytes.includes(title));
    assert.ok(!bytes.includes('Hello, README'));
    assert.equal(s.w.document.getElementById('statusText').textContent, 'SVG downloaded.');
  } finally { s.dom.window.close(); }
});

test('obsolete preview success/error cannot win during debounce, after newer success, or after kind changes', async () => {
  const s = await studio(base, { controlled: true });
  try {
    const initial = s.probes.at(-1);
    s.field('text', 'first edit');
    initial.onload(); initial.onerror();
    assertPending(s);
    s.flushRender();
    const first = s.probes.at(-1);
    s.field('text', 'latest edit');
    s.flushRender();
    const latest = s.probes.at(-1);
    latest.onload();
    const url = s.w.document.getElementById('previewImg').src;
    first.onload(); initial.onerror();
    assert.equal(s.w.document.getElementById('previewImg').src, url);
    assert.equal(new URL(url).searchParams.get('text'), 'latest edit');
    s.field('text', 'pending before kind switch');
    s.kind('badge');
    assertPending(s);
    latest.onload(); first.onerror();
    assertPending(s);
    s.probes.at(-1).onload();
    assert.ok(s.w.document.getElementById('previewImg').src.includes('/badge/'));
    assert.throws(s.flushRender, /debounced render scheduled/, 'kind switch cancels the queued old refresh');
  } finally { s.dom.window.close(); }
});

test('numeric edits stay unavailable on latest preview failure; retry renders latest values, not an old success', async () => {
  const s = await studio(base, { controlled: true });
  try {
    const initial = s.probes.at(-1);
    initial.onload();
    s.field('width', '901');
    assertPending(s);
    s.flushRender();
    const older = s.probes.at(-1);
    s.field('width', '997');
    older.onload();
    assertPending(s);
    s.flushRender();
    s.probes.at(-1).onerror();
    assertPending(s, 'error');
    assert.equal(s.w.document.getElementById('retryBtn').hidden, false);
    initial.onload(); older.onload();
    s.click('#downloadBtn');
    assertPending(s, 'error');
    assert.equal(s.requests.length, 0);
    s.click('#retryBtn');
    assertPending(s);
    assert.equal(new URL(s.probes.at(-1).url).searchParams.get('width'), '997');
    s.probes.at(-1).onload();
    s.click('#downloadBtn'); await pause(0);
    assert.equal(new URL(s.requests.at(-1)).searchParams.get('width'), '997');
    assert.equal(s.saved.length, 1);
  } finally { s.dom.window.close(); }
});

for (const stage of ['response', 'body', 'failure']) {
  test(`an edit or kind switch during export ${stage} discards stale bytes and status`, async () => {
    const pending = deferred();
    const s = await studio(base, { controlled: true, fetchSvg: url => stage === 'body'
      ? svgResponse(url, pending.promise) : pending.promise });
    try {
      s.probes.at(-1).onload();
      s.click('#downloadBtn'); s.click('#downloadBtn');
      assert.equal(s.requests.length, 1, 'duplicate download is disabled');
      await pause(0); // response consumed, body may remain pending
      s.field('text', 'latest draft');
      s.kind('score');
      s.probes.at(-1).onerror();
      assertPending(s, 'error');
      const status = s.w.document.getElementById('statusText').textContent;
      if (stage === 'failure') pending.reject(new Error('offline'));
      else pending.resolve(stage === 'body' ? new Blob(['<svg><text>old</text></svg>']) : svgResponse(s.requests[0]));
      await pause(0);
      assert.equal(s.saved.length, 0);
      assertPending(s, 'error');
      assert.equal(s.w.document.getElementById('statusText').textContent, status);
    } finally { s.dom.window.close(); }
  });
}

test('failed current export stays disabled until a successful preview retry', async () => {
  const s = await studio(base, { controlled: true, fetchSvg: () => ({ ok: false }) });
  try {
    s.probes.at(-1).onload();
    s.click('#downloadBtn'); await pause(0);
    assertPending(s, 'error');
    s.click('#downloadBtn');
    assert.equal(s.requests.length, 1);
    s.click('#retryBtn');
    assertPending(s);
    s.probes.at(-1).onload();
    assert.equal(s.w.document.getElementById('downloadBtn').disabled, false);
  } finally { s.dom.window.close(); }
});

test('all eight kinds preview and download through the same readiness gate', async () => {
  const s = await studio(base, { controlled: true });
  try {
    for (const kind of ['banner', 'badge', 'typing', 'icons', 'stats', 'profile', 'score', 'deploy']) {
      s.kind(kind);
      assertPending(s);
      s.probes.at(-1).onload();
      const preview = s.w.document.getElementById('previewImg').src;
      for (const format of ['md', 'html']) {
        assertImage(await s.copy(format), format, s.w.document.getElementById('previewImg').alt, preview);
      }
      assert.equal(await s.copy('url'), preview);
      s.click('#downloadBtn'); await pause(0);
      assert.equal(s.requests.at(-1), preview);
      assert.equal(s.saved.at(-1).name, `mark-${kind}.svg`);
      const svg = new JSDOM(await s.saved.at(-1).blob.text(), { contentType: 'image/svg+xml' });
      assert.equal(svg.window.document.documentElement.localName, 'svg');
      svg.window.close();
    }
    assert.equal(s.saved.length, 8);
  } finally { s.dom.window.close(); }
});
