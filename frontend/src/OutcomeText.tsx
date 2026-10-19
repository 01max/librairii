import {t, currentLanguage} from './i18n';
import en from '../../internal/localization/locales/en.json';
import {useLanguage} from './use-language';
export function outcomeText(text: string): string {
    if (currentLanguage() === 'en' || Object.hasOwn(en, text)) return t(text);
    return t('The action could not be completed');
}
export default function OutcomeText({text}: {text: string}) {
    useLanguage();
    const summary = outcomeText(text);
    return <>{summary}{summary !== text && !Object.hasOwn(en, text) && (
        <details><summary>{t('Technical details')}</summary><p>{text}</p></details>
    )}</>;
}
