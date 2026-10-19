import {t} from './i18n';
import {type PropsWithChildren, useEffect, useRef} from 'react';
import {useModalFocus} from './modal-focus';

export default function LibrarySidebar({children, open, onClose, suspended, inert = false}: PropsWithChildren<{
    open: boolean;
    onClose: () => void;
    suspended: boolean;
    inert?: boolean;
}>) {
    const panel = useRef<HTMLElement>(null);
    const closeButton = useRef<HTMLButtonElement>(null);
    useModalFocus(panel, closeButton, open, suspended);

    useEffect(() => {
        if (!open) return;
        const closeOnResize = () => {
            if (window.innerWidth > 850) onClose();
        };
        const closeOnEscape = (event: KeyboardEvent) => {
            if (event.key === 'Escape' && !suspended) {
                event.preventDefault();
                onClose();
            }
        };
        window.addEventListener('resize', closeOnResize);
        window.addEventListener('keydown', closeOnEscape);
        return () => {
            window.removeEventListener('resize', closeOnResize);
            window.removeEventListener('keydown', closeOnEscape);
        };
    }, [open, onClose, suspended]);

    return (
        <>
            {open && <div className="sidebar-backdrop" onClick={onClose} aria-hidden="true"/>}
            <aside ref={panel} id="library-sidebar" inert={inert}
                className={`filters${open ? ' sidebar-open' : ''}`}
                role={open ? 'dialog' : undefined}
                aria-modal={open ? true : undefined}
                aria-label={t("Saved shelves and refinements")}>
                <button ref={closeButton} className="sidebar-close" type="button"
                    onClick={onClose} aria-label={t("Close shelves and filters")}>{t("Done") + ' '}<span aria-hidden="true">×</span></button>
                {children}
            </aside>
        </>
    );
}
