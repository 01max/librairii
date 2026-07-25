import {type RefObject, useEffect} from 'react';

const focusableSelector = [
    'button:not([disabled])',
    'input:not([disabled])',
    'select:not([disabled])',
    'textarea:not([disabled])',
    'a[href]',
    '[tabindex]:not([tabindex="-1"])',
].join(',');

export function useModalFocus(
    container: RefObject<HTMLElement | null>,
    initial: RefObject<HTMLElement | null>,
    active = true,
) {
    useEffect(() => {
        if (!active) {
            return;
        }
        const opener = document.activeElement instanceof HTMLElement
            ? document.activeElement
            : null;
        const focusInitial = window.setTimeout(() => {
            initial.current?.focus();
        }, 0);
        const trapTab = (event: KeyboardEvent) => {
            if (event.key !== 'Tab' || !container.current) {
                return;
            }
            const focusable = [...container.current.querySelectorAll<HTMLElement>(
                focusableSelector,
            )].filter((element) => !element.hidden);
            if (focusable.length === 0) {
                event.preventDefault();
                return;
            }
            const first = focusable[0];
            const last = focusable[focusable.length - 1];
            if (event.shiftKey && document.activeElement === first) {
                event.preventDefault();
                last.focus();
            } else if (!event.shiftKey && document.activeElement === last) {
                event.preventDefault();
                first.focus();
            } else if (!container.current.contains(document.activeElement)) {
                event.preventDefault();
                first.focus();
            }
        };
        document.addEventListener('keydown', trapTab);
        return () => {
            window.clearTimeout(focusInitial);
            document.removeEventListener('keydown', trapTab);
            opener?.focus();
        };
    }, [active, container, initial]);
}
