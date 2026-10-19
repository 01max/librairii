import {useEffect, useRef, useState} from 'react';
import {Settings, SetUILanguage, SelectLibraryLocation, ConfirmLibrarySetup, StartLibraryMove, CancelLibraryMove, RetryLibrary, LocateLibrary} from '../wailsjs/go/main/App';
import type {main} from '../wailsjs/go/models';
import {EventsOn} from '../wailsjs/runtime/runtime';
import {setLanguage, t, number, type Language} from './i18n';
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
 const lastMove = useRef('');
 const moving = !!snapshot?.move && !['succeeded','failed','cancelled','cleanup_pending'].includes(snapshot.move.phase);
    const dialog = useRef<HTMLElement>(null);
    const close = useRef<HTMLButtonElement>(null);
    useEffect(() => {
        if (!open) return;
        const root=dialog.current?.closest('.app');
        if (!root) return;
        const siblings=[...root.children].filter((child): child is HTMLElement => child instanceof HTMLElement && !child.contains(dialog.current));
        const previous=siblings.map((child) => child.inert);
        siblings.forEach((child) => {child.inert=true;});
        return () => siblings.forEach((child,index) => {child.inert=previous[index];});
    },[open]);
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
            if (event.key === 'Escape' && !working && !moving && !snapshot?.setupRequired) { event.stopImmediatePropagation(); setOpen(false); }
        };
        window.addEventListener('keydown', escape, true);
        return () => window.removeEventListener('keydown', escape, true);
    }, [open, working, moving, snapshot?.setupRequired]);
    useEffect(() => {
        if (!snapshot?.move) return;
        const phase = snapshot.move.phase;
        if (['succeeded','failed','cancelled','cleanup_pending'].includes(phase)) {
            if (lastMove.current !== phase) { lastMove.current = phase; onReady(); }
            return;
        }
        lastMove.current = phase;
        const timer = window.setInterval(() => { void Settings().then(accept).catch(() => setError('settings.invalid')); }, 200);
        return () => window.clearInterval(timer);
    }, [snapshot?.move?.phase, onReady]);
    async function update(action: () => Promise<Snapshot>) {
        setWorking(true); setError('');
        try { accept(await action()); } catch { setError('settings.invalid'); } finally { setWorking(false); }
    }
    return <>
        <button type="button" className="settings-entry" onClick={() => { setOpen(true); if (nativeSettingsAvailable()) void update(Settings); }} aria-label={t('Settings')}>⚙</button>
        {open && <ModalBackdrop onDismiss={() => setOpen(false)} disabled={working || moving || snapshot?.setupRequired} className="settings-backdrop">
            <section ref={dialog} className="detail-dialog settings-panel" role="dialog" aria-modal="true" aria-labelledby="settings-title">
                <h2 id="settings-title">{t(snapshot?.setupRequired ? 'Welcome to Librairii' : 'Settings')}</h2>
 {snapshot?.setupRequired && <p>{t('Choose where to keep your library.')}</p>}
                <label>{t('Interface language')}
                    <select aria-label={t('Interface language')} value={snapshot?.language ?? 'en'} disabled={working || !snapshot}
                        onChange={(event) => { const next = event.currentTarget.value as Language; void update(() => SetUILanguage(next)); }}>
                        <option value="fr">Français</option><option value="en">English</option>
                    </select>
                </label>
                <h3>{t('Stories library')}</h3>
                <p className="library-location">{snapshot?.root || '—'}</p>
                {snapshot?.isolated ? <p>{t('Location is controlled by LIBRAIRII_DATA_ROOT.')}</p> :
                    <button type="button" disabled={working || moving || !snapshot} onClick={() => void update(SelectLibraryLocation)}>{t('Change location…')}</button>}
                {snapshot?.recovery && <div>
                    <p>{t('Library unavailable')}</p>
                    <button type="button" disabled={working} onClick={() => void update(async () => { const result=await RetryLibrary(); if (!result.recovery) {onReady();setOpen(false);}return result; })}>{t('Try again')}</button>
                    <button type="button" disabled={working} onClick={() => void update(async () => { const result=await LocateLibrary(); if (!result.recovery) {onReady();setOpen(false);}return result; })}>{t('Locate library…')}</button>
                </div>}
                {snapshot?.preparation && !moving && <div className="move-preview">
                    <p>{t('All library files will be moved.')}</p>
                    <p>{snapshot.preparation.source} → {snapshot.preparation.destination}</p>
                    <p>{t('Total size')}: {number(snapshot.preparation.bytes)} B</p>
                    <button type="button" disabled={working} onClick={() => void update(() => StartLibraryMove(snapshot.preparation!.id))}>{t('Move library')}</button>
                </div>}
                {snapshot?.move && <div role="status" aria-live="polite">
                    <p>{t(({copying:'Moving library…',verifying:'Verification',committing:'Moving library…',cleanup:'Moving library…',succeeded:'Library moved successfully.',failed:'settings.move_failed',cancelled:'Cancelled',cleanup_pending:'Previous location needs cleanup.'} as Record<string,string>)[snapshot.move.phase] ?? 'Moving library…')}</p>
                    {moving && <><progress max={Math.max(1,snapshot.move.total)} value={snapshot.move.copied}/><p>{number(snapshot.move.copied)} / {number(snapshot.move.total)} B</p></>}
                    {snapshot.move.cleanupPath && <p className="library-location">{snapshot.move.cleanupPath}</p>}
                    {snapshot.move.error && <p>{t(snapshot.move.error)}</p>}
                    {snapshot.move.cancelable && <button type="button" onClick={() => void update(CancelLibraryMove)}>{t('Cancel')}</button>}
                </div>}
                {error && <p role="alert">{t(error)}</p>}
                <div className="dialog-actions">
 {snapshot?.setupRequired && <button type="button" disabled={working} onClick={() => {
  setWorking(true); setError('');
  void ConfirmLibrarySetup().then(async (result) => {
   if (result.error) { setError(result.error.message); return; }
   if (!result.success) { setError('settings.invalid'); return; }
   accept(await Settings()); setOpen(false); onReady();
  }).catch(() => setError('settings.invalid')).finally(() => setWorking(false));
 }}>{t('Use this folder')}</button>}
 <button ref={close} type="button" disabled={working || moving || snapshot?.setupRequired} onClick={() => { setOpen(false); }}>{t('Close')}</button></div>
            </section>
        </ModalBackdrop>}
    </>;
}
