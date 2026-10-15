import {expect, test} from 'vitest';
import {readableStoryDescription} from './story-description';

test('turns catalog markup into readable paragraphs and decodes entities', () => {
    expect(readableStoryDescription(
        'Un dragon !<br/><br/>Aide <b>Lou</b> &amp; ses amis.<span class="SF_splitter"></span>',
    )).toBe('Un dragon !\n\nAide Lou & ses amis.');
    expect(readableStoryDescription('<p>First&nbsp;story.</p><p>Second story.</p>'))
        .toBe('First story.\n\nSecond story.');
});

test('preserves plain text and readable list items', () => {
    expect(readableStoryDescription('For ages 3 < 5.\nTwo adventures.'))
        .toBe('For ages 3 < 5.\nTwo adventures.');
    expect(readableStoryDescription('<ul><li>Lou</li><li>Dragon</li></ul>'))
        .toBe('• Lou\n\n• Dragon');
    expect(readableStoryDescription()).toBe('');
    expect(readableStoryDescription('<span></span><br/>')).toBe('');
});

test('discards active markup without creating live elements', () => {
    expect(readableStoryDescription(
        '<script>alert(1)</script><style>body{display:none}</style>' +
        '<iframe src="https://example.com">hidden</iframe>' +
        '<img src="missing" onerror="alert(1)">' +
        '<a href="javascript:alert(1)">A story</a>',
    )).toBe('A story');
    expect(document.querySelector('iframe, img, script')).toBeNull();
});
