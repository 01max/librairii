import {afterEach, expect, test} from 'vitest';
import en from '../../internal/localization/locales/en.json';
import fr from '../../internal/localization/locales/fr.json';
import {message, plural, setLanguage} from './i18n';
afterEach(() => setLanguage('en'));
test('catalogs have identical message keys and interpolation parameters', () => {
 expect(Object.keys(fr).sort()).toEqual(Object.keys(en).sort());
 for (const key of Object.keys(en) as Array<keyof typeof en>) {
  expect((fr[key].match(/\{\d+\}/g) ?? []).sort(),key).toEqual((en[key].match(/\{\d+\}/g) ?? []).sort());
 }
});
test('French interpolation and plural rules', () => {
 setLanguage('fr'); expect(message('Close {0}', 'Alice')).toBe('Fermer Alice');
 expect(plural(0,'story','stories')).toBe('histoire'); expect(plural(2,'story','stories')).toBe('histoires');
});
