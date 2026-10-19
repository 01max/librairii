import {t} from './i18n';

export default function BrokenToggle({title, broken, disabled, onChange}: {
    title: string; broken: boolean; disabled: boolean; onChange: (broken: boolean) => void;
}) {
    return <label className="broken-toggle">
        <span>{t('Broken')}</span>
        <button type="button" role="switch" aria-label={t('Broken: {0}', title)} aria-checked={broken}
            disabled={disabled} onClick={() => onChange(!broken)}><span aria-hidden="true"/></button>
    </label>;
}
