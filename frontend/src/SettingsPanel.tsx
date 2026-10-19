import {useEffect, useRef, useState} from 'react';
import {Settings, SetUILanguage, SelectLibraryLocation} from '../wailsjs/go/main/App';
import type {main} from '../wailsjs/go/models';
import {EventsOn} from '../wailsjs/runtime/runtime';
import {setLanguage, t, type Language} from './i18n';
import {useLanguage} from './use-language';
import {useModalFocus} from './modal-focus';
import ModalBackdrop from './ModalBackdrop';

type Snapshot = main.SettingsSnapshot;
function nativeSettingsAvailable() {
    return typeof (window as unknown as {go?: {main?: {App?: {Settings?: unknown}}}}).go?.main?.App?.Settings === 'function';
}
export default function SettingsPanel({onReady}: {onReady: () => void}) {
    useLanguage();
    const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
    const [open, setOpen] = useState(false);
    const [working, setWorking] = useState(false);
    const [error, setError] = useState('');
    const dialog = useRef<HTMLElement>(null);
    const close = useRef<HTMLButtonElement>(null);
    useModalFocus(dialog, close, open);
    function accept(value: Snapshot) {
        setSnapshot(value);
        if (value.language === 'fr' || value.language === 'en') setLanguage(value.language);
        if (value.pendingOpen || value.setupRequired) setOpen(true);
        setError(value.error ?? '');
    }
    useEffect(() => {
        if (!nativeSettingsAvailable()) return;
        let active = true;
        const refresh = () => { void Settings().then((value) => { if (active) accept(value); }).catch(() => { if (active) setError('settings.invalid'); }); };
        const unsubscribe = EventsOn('settings:open', () => { setOpen(true); refresh(); });
        refresh();
        return () => { active = false; unsubscribe(); };
    }, []);
    useEffect(() => {
        if (!open) return;
        const escape = (event: KeyboardEvent) => {
            if (event.key === 'Escape' && !working && !snapshot?.setupRequired) { event.stopImmediatePropagation(); setOpen(false); }
        };
        window.addEventListener('keydown', escape, true);
        return () => window.removeEventListener('keydown', escape, true);
    }, [open, working, snapshot?.setupRequired]);
    async function update(action: () => Promise<Snapshot>) {
        setWorking(true); setError('');
        try { accept(await action()); } catch { setError('settings.invalid'); } finally { setWorking(false); }
    }
    return <>
        <button type="button" className="settings-entry" onClick={() => { setOpen(true); if (nativeSettingsAvailable()) void update(Settings); }} aria-label={t('Settings')}>⚙</button>
        {open && <ModalBackdrop onDismiss={() => setOpen(false)} disabled={working || snapshot?.setupRequired} className="settings-backdrop">
            <section ref={dialog} className="detail-dialog settings-panel" role="dialog" aria-modal="true" aria-labelledby="settings-title">
                <h2 id="settings-title">{t('Settings')}</h2>
                <label>{t('Interface language')}
                    <select aria-label={t('Interface language')} value={snapshot?.language ?? 'en'} disabled={working || !snapshot}
                        onChange={(event) => { const next = event.currentTarget.value as Language; void update(() => SetUILanguage(next)); }}>
                        <option value="fr">Français</option><option value="en">English</option>
                    </select>
                </label>
                <h3>{t('Stories library')}</h3>
                <p className="library-location">{snapshot?.root || '—'}</p>
                {snapshot?.isolated ? <p>{t('Location is controlled by LIBRAIRII_DATA_ROOT.')}</p> :
                    <button type="button" disabled={working || !snapshot} onClick={() => void update(SelectLibraryLocation)}>{t('Change location…')}</button>}
                {error && <p role="alert">{t(error)}</p>}
                <div className="dialog-actions"><button ref={close} type="button" disabled={working} onClick={() => { setOpen(false); }}>{t('Close')}</button></div>
            </section>
        </ModalBackdrop>}
    </>;
}
