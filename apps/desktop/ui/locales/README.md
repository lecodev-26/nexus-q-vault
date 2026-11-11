# UI translations

The desktop UI keeps its locale dictionaries in this directory:

- `en.json` is the canonical source dictionary. Its keys are the English source strings.
- `es.json` contains the Spanish values for those same keys.
- `ca.json` contains the Catalan values for those same keys.
- `fr.json` contains the French values for those same keys.

The shared runtime is `../i18n/index.js`. It loads the dictionaries, remembers the selected language through the app's existing local preference, and translates static text, placeholders, titles and accessible labels. Dynamic UI messages should use `NexusQI18n.t("English source string")` rather than language-specific conditionals.

## Adding a language

1. Copy `en.json` to a new locale file.
2. Translate every value while keeping every English key unchanged.
3. Add the language code and its display name to `manifest.json`. The selector is populated automatically.
4. Run `node --test apps/desktop/ui/i18n/i18n.test.js` and check that every locale has identical keys and no empty or untranslated values.

No translation dependency or external service is required at runtime.
