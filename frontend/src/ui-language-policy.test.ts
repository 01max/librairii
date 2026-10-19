import {afterEach, expect, test} from 'vitest';
import {setLanguage, t} from './i18n';
import {encodeCollectionQuery, DEFAULT_COLLECTION_QUERY} from './query-codec';
afterEach(() => setLanguage('en'));
test('language changes presentation without changing a stored collection query or authored content', () => {
 const story = {title: 'Les aventures de Suzanne', description: 'Une histoire française.', shelf: 'Bedtime', tag: 'Mes favoris'};
 const before = JSON.stringify(story);
 const query = {...DEFAULT_COLLECTION_QUERY, name: story.title, languages: ['fr-FR']};
 const encoded = encodeCollectionQuery(query);
 setLanguage('fr');
 expect(t('All stories')).toBe('Toutes les histoires');
 expect(JSON.stringify(story)).toBe(before);
 expect(encodeCollectionQuery(query)).toBe(encoded);
});
