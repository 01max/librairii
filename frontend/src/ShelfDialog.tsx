import {type PropsWithChildren, useEffect, useRef} from 'react';
import ModalBackdrop from './ModalBackdrop';
import {useModalFocus} from './modal-focus';
import {t} from './i18n';

export default function ShelfDialog({title, busy, onClose, children}: PropsWithChildren<{
    title: string; busy: boolean; onClose: () => void;
}>) {
    const panel = useRef<HTMLElement>(null);
    const close = useRef<HTMLButtonElement>(null);
    useModalFocus(panel, close);
    useEffect(() => {
        const escape = (event: KeyboardEvent) => { if (event.key === 'Escape' && !busy) { event.stopImmediatePropagation(); onClose(); } };
        document.addEventListener('keydown', escape, true);
        return () => document.removeEventListener('keydown', escape, true);
    }, [busy, onClose]);
    return <ModalBackdrop onDismiss={onClose} disabled={busy}>
        <section ref={panel} className="detail-dialog curated-dialog" role="dialog" aria-modal="true" aria-label={title}>
            <div className="dialog-kicker">{t('Shelves')}</div><h3>{title}</h3>
            <button ref={close} type="button" disabled={busy} onClick={onClose}>{t('Cancel')}</button>
            {children}
        </section>
    </ModalBackdrop>;
}
