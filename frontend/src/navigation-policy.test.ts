import {afterEach, describe, expect, test} from 'vitest';
import {
    installExternalNavigationGuard,
    navigationLeavesCurrentDocument,
} from './navigation-policy';

describe('release navigation policy', () => {
    const current = 'http://wails.localhost/index.html?fixture=collection';
    let removeGuard: (() => void) | undefined;

    afterEach(() => {
        removeGuard?.();
        removeGuard = undefined;
        document.body.replaceChildren();
    });

    test.each([
        ['https://example.com', true],
        ['mailto:support@example.com', true],
        ['file:///private/archive.pk', true],
        ['/other.html', true],
        ['?fixture=other', true],
        ['#story-42', false],
        ['index.html?fixture=collection#story-42', false],
    ])(
        'classifies %s as leaving the document: %s',
        (destination, expected) => {
            expect(
                navigationLeavesCurrentDocument(destination, current),
            ).toBe(expected);
        },
    );

    test('prevents external, new-window, download, and form navigation', () => {
        document.body.innerHTML = `
            <a id="external" href="https://example.com"><span>External</span></a>
            <a id="new-window" href="#story-42" target="_blank">New window</a>
            <a id="download" href="#story-42" download>Download</a>
            <form id="form"><button type="submit">Submit</button></form>
        `;
        removeGuard = installExternalNavigationGuard(document, () => current);

        const dispatchClick = (selector: string) => {
            const target = document.querySelector(selector);
            if (target === null) {
                throw new Error(`missing click target: ${selector}`);
            }
            const event = new MouseEvent('click', {
                bubbles: true,
                cancelable: true,
            });
            target.dispatchEvent(event);
            return event.defaultPrevented;
        };

        expect(dispatchClick('#external span')).toBe(true);
        expect(dispatchClick('#new-window')).toBe(true);
        expect(dispatchClick('#download')).toBe(true);

        const form = document.querySelector<HTMLFormElement>('#form');
        if (form === null) {
            throw new Error('missing form');
        }
        const submit = new SubmitEvent('submit', {
            bubbles: true,
            cancelable: true,
        });
        form.dispatchEvent(submit);
        expect(submit.defaultPrevented).toBe(true);
    });

    test('removes its listeners during teardown', () => {
        document.body.innerHTML = '<a href="https://example.com">External</a>';
        removeGuard = installExternalNavigationGuard(document, () => current);
        removeGuard();
        removeGuard = undefined;

        const event = new MouseEvent('click', {
            bubbles: true,
            cancelable: true,
        });
        let preventedBeforeFallback = true;
        const preventJSDOMNavigation = (click: Event) => {
            preventedBeforeFallback = click.defaultPrevented;
            click.preventDefault();
        };
        document.addEventListener('click', preventJSDOMNavigation, {
            once: true,
        });
        document.querySelector('a')?.dispatchEvent(event);

        expect(preventedBeforeFallback).toBe(false);
    });
});
