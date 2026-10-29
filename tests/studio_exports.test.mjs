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
async function studio(url = base) {
  const dom = new JSDOM(html, { url, runScripts: 'outside-only' });
  const { window: w } = dom;
  const copied = [], saved = [];
  const svg = '<svg xmlns="http://www.w3.org/2000/svg" width="880" height="220"/>';
  w.fetch = async url => String(url).includes('catalog')
    ? { ok: true, json: async () => ({}) }
    : { ok: true, headers: new Map([['content-type', 'image/svg+xml']]), blob: async () => new Blob([svg]) };
  w.Image = class {
    naturalWidth = 880;
    naturalHeight = 220;
    set src(value) { queueMicrotask(() => this.onload()); }
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
  return { w, dom, field, kind, click, copy, saved };
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
