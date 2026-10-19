import {afterEach, expect, test} from 'vitest';
import {setLanguage} from './i18n';
import {outcomeKey, outcomeText} from './outcome-message';

afterEach(() => setLanguage('en'));

test.each([
    ['Story imported.', 'Histoire importée.'],
    ['Story archive exported.', 'Archive de l’histoire exportée.'],
    ['Official metadata refreshed; 1234 local stories matched.', 'Métadonnées officielles actualisées ; 1\u202f234 histoires locales associées.'],
])('translates the current successful outcome %s', (message, translated) => {
    setLanguage('fr');
    expect(outcomeText(message, 'succeeded')).toBe(translated);
    expect(outcomeKey(message)).toBeDefined();
    setLanguage('en');
    expect(outcomeText(message, 'succeeded')).toBe(message.replace('1234', '1,234'));
});

test.each([
    ['succeeded', 'Terminé'], ['partially_succeeded', 'Terminé avec des exceptions'],
    ['skipped', 'Ignoré'], ['cancelled', 'Annulé'], ['conflicted', 'En conflit'],
    ['running', 'En cours'], ['failed', 'L’action n’a pas pu être effectuée'],
])('retains %s status for unknown diagnostics', (status, translated) => {
    setLanguage('fr');
    expect(outcomeText('Unknown diagnostic', status)).toBe(translated);
    setLanguage('en');
    expect(outcomeText('Unknown diagnostic', status)).toBe('Unknown diagnostic');
});
