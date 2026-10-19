import {t} from './i18n';

export default function StoryFlagButton({title, broken, disabled, onChange}: {
    title: string;
    broken: boolean;
    disabled: boolean;
    onChange: (broken: boolean) => void;
}) {
    const label = broken ? t('Clear broken flag for {0}', title) : t('Mark {0} as broken', title);
    return <button className="story-flag-action" type="button" disabled={disabled}
        aria-label={label} title={label} aria-pressed={broken}
        onClick={() => onChange(!broken)}>
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
            <path d="M5 21V4m0 0c5-5 9 5 15 0v10c-6 5-10-5-15 0"/>
        </svg>
    </button>;
}
