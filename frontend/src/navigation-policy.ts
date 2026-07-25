const selfTargetNames = new Set(['', '_self']);

export function navigationLeavesCurrentDocument(
    destination: string,
    currentHref: string,
): boolean {
    if (destination.trimStart().startsWith('#')) {
        return false;
    }
    try {
        const current = new URL(currentHref);
        const target = new URL(destination, current);
        return (
            target.protocol !== current.protocol ||
            target.origin !== current.origin ||
            target.pathname !== current.pathname ||
            target.search !== current.search
        );
    } catch {
        return true;
    }
}

export function installExternalNavigationGuard(
    documentRoot: Document = document,
    currentHref: () => string = () => window.location.href,
): () => void {
    const guardClick = (event: MouseEvent) => {
        const target = event.target;
        if (!(target instanceof Element)) {
            return;
        }
        const anchor = target.closest<HTMLAnchorElement>('a[href]');
        if (anchor === null) {
            return;
        }
        const destination = anchor.getAttribute('href') ?? '';
        const namedTarget = anchor.getAttribute('target')?.toLowerCase() ?? '';
        if (
            !selfTargetNames.has(namedTarget) ||
            anchor.hasAttribute('download') ||
            navigationLeavesCurrentDocument(destination, currentHref())
        ) {
            event.preventDefault();
        }
    };
    const guardSubmit = (event: SubmitEvent) => {
        event.preventDefault();
    };

    documentRoot.addEventListener('click', guardClick, true);
    documentRoot.addEventListener('submit', guardSubmit, true);
    return () => {
        documentRoot.removeEventListener('click', guardClick, true);
        documentRoot.removeEventListener('submit', guardSubmit, true);
    };
}
