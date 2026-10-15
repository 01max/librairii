const omittedElements = new Set([
    'SCRIPT', 'STYLE', 'IFRAME', 'OBJECT', 'TEMPLATE', 'NOSCRIPT',
]);
const paragraphElements = new Set([
    'P', 'DIV', 'SECTION', 'ARTICLE', 'H1', 'H2', 'H3', 'H4', 'H5', 'H6',
    'BLOCKQUOTE', 'UL', 'OL',
]);

// Parse in an inert template and return text only. Metadata markup never enters
// the live document; paragraph breaks and decoded entities remain readable.
export function readableStoryDescription(description?: string): string {
    if (!description) {
        return '';
    }
    const template = document.createElement('template');
    template.innerHTML = description;
    const textOf = (node: Node): string => {
        if (node.nodeType === Node.TEXT_NODE) {
            return node.textContent ?? '';
        }
        const tag = node.nodeName;
        if (omittedElements.has(tag)) {
            return '';
        }
        if (tag === 'BR') {
            return '\n';
        }
        const text = [...node.childNodes].map(textOf).join('');
        if (tag === 'LI') {
            return `\n• ${text.trim()}\n`;
        }
        return paragraphElements.has(tag) ? `\n\n${text}\n\n` : text;
    };
    return textOf(template.content)
        .replace(/[^\S\n]+/g, ' ')
        .replace(/ *\n */g, '\n')
        .replace(/\n{3,}/g, '\n\n')
        .trim();
}
