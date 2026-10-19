import {t, currentLanguage} from './i18n';
import en from '../../internal/localization/locales/en.json';
import fr from '../../internal/localization/locales/fr.json';
const historicalKeys = new Map(Object.entries(fr).map(([key, value]) => [value, key]));
export function outcomeKey(text: string): string | undefined {
    return Object.hasOwn(en, text) ? text : historicalKeys.get(text);
}
export function outcomeText(text: string): string {
    const key=outcomeKey(text);
    if (key) return t(key);
    return currentLanguage() === 'en' ? text : t('The action could not be completed');
}
