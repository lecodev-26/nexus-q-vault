const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const locales = path.join(__dirname, '..', 'locales');
const en = require(path.join(locales, 'en.json'));
const es = require(path.join(locales, 'es.json'));
const ca = require(path.join(locales, 'ca.json'));
const fr = require(path.join(locales, 'fr.json'));
const manifest = require(path.join(locales, 'manifest.json'));
test('manifest registers every locale dictionary', () => {
  assert.deepEqual(Object.keys(manifest).sort(), ['ca', 'en', 'es', 'fr']);
  for (const code of Object.keys(manifest)) assert.ok(fs.existsSync(path.join(locales, `${code}.json`)));
});
test('English and Spanish dictionaries have identical keys', () => {
  assert.deepEqual(Object.keys(es).sort(), Object.keys(en).sort());
});
test('Catalan and French dictionaries have complete English key coverage', () => {
  assert.deepEqual(Object.keys(ca).sort(), Object.keys(en).sort());
  assert.deepEqual(Object.keys(fr).sort(), Object.keys(en).sort());
});
test('every Catalan and French entry maps to a known English key and has a value', () => {
  for (const [locale, dictionary] of Object.entries({ ca, fr })) {
    for (const [key, value] of Object.entries(dictionary)) {
      assert.ok(Object.hasOwn(en, key), `Unknown ${locale} key: ${key}`);
      assert.equal(typeof value, 'string');
      assert.ok(value.trim().length > 0, `Empty ${locale} value: ${key}`);
    }
  }
});
test('all locale files are valid JSON', () => {
  for (const file of ['manifest.json', 'en.json', 'es.json', 'ca.json', 'fr.json']) assert.doesNotThrow(() => JSON.parse(fs.readFileSync(path.join(locales, file), 'utf8')));
});
test('Catalan and French values are translated rather than silently copied from English', () => {
  for (const key of Object.keys(en)) {
    assert.notEqual(ca[key], en[key], `Missing Catalan translation: ${key}`);
    assert.notEqual(fr[key], en[key], `Missing French translation: ${key}`);
  }
});
test('the application uses the shared locale runtime instead of language ternaries', () => {
  const app = fs.readFileSync(path.join(__dirname, '..', 'app.js'), 'utf8');
  assert.match(app, /NexusQI18n\.t/);
  assert.doesNotMatch(app, /const translations\s*=/);
  assert.doesNotMatch(app, /preference\("language"[^\n]*===\s*"es"/);
});
test('locale runtime loads Catalan and translates backend error prefixes', async () => {
  const vm = require('node:vm');
  const runtime = fs.readFileSync(path.join(__dirname, 'index.js'), 'utf8');
  const window = {};
  const document = {
    body: {},
    documentElement: {},
    getElementById: () => null,
    createTreeWalker: () => ({ nextNode: () => null }),
    querySelectorAll: () => [],
  };
  vm.runInNewContext(runtime, {
    window,
    document,
    NodeFilter: { SHOW_TEXT: 4 },
    fetch: async (url) => ({
      ok: true,
      json: async () => JSON.parse(fs.readFileSync(path.join(locales, path.basename(url)), 'utf8')),
    }),
  });
  await window.NexusQI18n.ready;
  await window.NexusQI18n.setLanguage('ca');
  assert.equal(window.NexusQI18n.t('Create a vault'), ca['Create a vault']);
  assert.equal(
    window.NexusQI18n.translateValue('No se pudo crear el vault: storage error'),
    'No s’ha pogut crear la caixa forta: storage error',
  );
  await window.NexusQI18n.setLanguage('fr');
  assert.equal(window.NexusQI18n.t('Create a vault'), fr['Create a vault']);
  assert.equal(
    window.NexusQI18n.translateValue('No se pudo crear el vault: storage error'),
    'Impossible de créer le coffre : storage error',
  );
});

test('Spanish HTML copy is represented in the locale dictionaries', () => {
  const html = fs.readFileSync(path.join(__dirname, '..', 'index.html'), 'utf8');
  const visibleText = [...html.matchAll(/>([^<>]+)</g)].map((match) => match[1].trim()).filter(Boolean);
  const known = new Set([...Object.keys(en), ...Object.values(en), ...Object.values(es)]);
  const ignored = new Set(['NEXUS-Q Vault', 'NEXUS-Q', 'VAULT DESKTOP', '◫', '▣', '⌘', '⚙', '?', 'U', 'Vault', '◈', '▤', '⌁', '01', '→', '02', '03', '04', '05', '06', 'NEXUS-Q Vault · Local-first · No telemetry']);
  const missing = [...new Set(visibleText.filter((value) => !known.has(value) && !ignored.has(value)))];
  assert.deepEqual(missing, []);
});
