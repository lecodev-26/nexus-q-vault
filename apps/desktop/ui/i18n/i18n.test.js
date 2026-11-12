const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const locales = path.join(__dirname, '..', 'locales');
const en = require(path.join(locales, 'en.json'));
const es = require(path.join(locales, 'es.json'));
const ca = require(path.join(locales, 'ca.json'));
const fr = require(path.join(locales, 'fr.json'));
const de = require(path.join(locales, 'de.json'));
const ptPT = require(path.join(locales, 'pt-PT.json'));
const ptBR = require(path.join(locales, 'pt-BR.json'));
const manifest = require(path.join(locales, 'manifest.json'));
test('manifest registers every locale dictionary', () => {
  assert.deepEqual(Object.keys(manifest).sort(), ['ca', 'de', 'en', 'es', 'fr', 'pt-BR', 'pt-PT']);
  for (const code of Object.keys(manifest)) assert.ok(fs.existsSync(path.join(locales, `${code}.json`)));
});
test('English and Spanish dictionaries have identical keys', () => {
  assert.deepEqual(Object.keys(es).sort(), Object.keys(en).sort());
});
test('all translated dictionaries have complete English key coverage', () => {
  for (const dictionary of [ca, fr, de, ptPT, ptBR]) assert.deepEqual(Object.keys(dictionary).sort(), Object.keys(en).sort());
});
test('every translated entry maps to a known English key and has a value', () => {
  for (const [locale, dictionary] of Object.entries({ ca, fr, de, 'pt-PT': ptPT, 'pt-BR': ptBR })) {
    for (const [key, value] of Object.entries(dictionary)) {
      assert.ok(Object.hasOwn(en, key), `Unknown ${locale} key: ${key}`);
      assert.equal(typeof value, 'string');
      assert.ok(value.trim().length > 0, `Empty ${locale} value: ${key}`);
    }
  }
});
test('all locale files are valid JSON', () => {
  for (const file of ['manifest.json', 'en.json', 'es.json', 'ca.json', 'fr.json', 'de.json', 'pt-PT.json', 'pt-BR.json']) assert.doesNotThrow(() => JSON.parse(fs.readFileSync(path.join(locales, file), 'utf8')));
});
test('translated locale values are not silently copied from English', () => {
  for (const key of Object.keys(en)) {
    assert.notEqual(ca[key], en[key], `Missing Catalan translation: ${key}`);
    assert.notEqual(fr[key], en[key], `Missing French translation: ${key}`);
    assert.notEqual(de[key], en[key], `Missing German translation: ${key}`);
    assert.notEqual(ptPT[key], en[key], `Missing pt-PT translation: ${key}`);
    assert.notEqual(ptBR[key], en[key], `Missing pt-BR translation: ${key}`);
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
  await window.NexusQI18n.setLanguage('de');
  assert.equal(window.NexusQI18n.t('Create a vault'), de['Create a vault']);
  assert.equal(
    window.NexusQI18n.translateValue('No se pudo crear el vault: storage error'),
    `${de['Could not create the vault: ']}storage error`,
  );
  await window.NexusQI18n.setLanguage('pt-PT');
  assert.equal(window.NexusQI18n.t('Create a vault'), ptPT['Create a vault']);
  assert.equal(window.NexusQI18n.translateValue('No se pudo crear el vault: storage error'), `${ptPT['Could not create the vault: ']}storage error`);
  await window.NexusQI18n.setLanguage('pt-BR');
  assert.equal(window.NexusQI18n.t('Create a vault'), ptBR['Create a vault']);
  assert.equal(window.NexusQI18n.translateValue('No se pudo crear el vault: storage error'), `${ptBR['Could not create the vault: ']}storage error`);
});

test('Spanish HTML copy is represented in the locale dictionaries', () => {
  const html = fs.readFileSync(path.join(__dirname, '..', 'index.html'), 'utf8');
  const visibleText = [...html.matchAll(/>([^<>]+)</g)].map((match) => match[1].trim()).filter(Boolean);
  const known = new Set([...Object.keys(en), ...Object.values(en), ...Object.values(es)]);
  const ignored = new Set(['NEXUS-Q Vault', 'NEXUS-Q', 'VAULT DESKTOP', '◫', '▣', '⌘', '⚙', '?', 'U', 'Vault', '◈', '▤', '⌁', '01', '→', '02', '03', '04', '05', '06', 'NEXUS-Q Vault · Local-first · No telemetry']);
  const missing = [...new Set(visibleText.filter((value) => !known.has(value) && !ignored.has(value)))];
  assert.deepEqual(missing, []);
});


test('Portuguese variants preserve technical placeholders and regional wording', () => {
  const placeholders = (value) => value.match(/\{[^}]+\}|%[sd]|\.nq[a-z]+|\b(?:MiB|KiB)\b/g) || [];
  for (const key of Object.keys(en)) {
    assert.deepEqual(placeholders(ptPT[key]), placeholders(en[key]), `pt-PT placeholders changed: ${key}`);
    assert.deepEqual(placeholders(ptBR[key]), placeholders(en[key]), `pt-BR placeholders changed: ${key}`);
  }
  assert.notEqual(ptPT['Password'], ptBR['Password']);
  assert.equal(ptPT['Save a backup'], 'Guardar uma cópia de segurança');
  assert.equal(ptPT['Receiving identity'], 'Identidade recetora');
  assert.equal(ptPT['Running'], 'Em execução');
  assert.equal(ptBR['Receiving identity'], 'Identidade de recebimento');
  assert.equal(ptBR['Running'], 'Em execução');
  assert.match(ptPT['Password'], /palavra-passe/i);
  assert.equal(ptBR['Password'], 'Senha');
  for (const [key, value] of Object.entries(ptPT)) assert.doesNotMatch(value, /\b(?:gerenciador|Salve-o|sotaque)\b/i, 'Brazilian wording in pt-PT: ' + key);
  for (const [key, value] of Object.entries(ptBR)) assert.doesNotMatch(value, /\b(?:ficheiro|palavra-passe)\b/i, 'European wording in pt-BR: ' + key);
});
