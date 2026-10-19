import en from '../../internal/localization/locales/en.json';
import fr from '../../internal/localization/locales/fr.json';
export type Language = 'en' | 'fr';
export type MessageKey = keyof typeof en;
let language: Language = 'en';
const listeners = new Set<() => void>();
export function currentLanguage() { return language; }
export function subscribeLanguage(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener); }; }
export function setLanguage(next: Language) {
    language = next;
    document.documentElement.lang = next;
    listeners.forEach((listener) => listener());
}
export function t(key: string, ...values: unknown[]): string {
    const catalog: Record<string, string> = language === 'fr' ? fr : en;
    const text = catalog[key] ?? key;
    return text.replace(/\{(\d+)\}/g, (_, index: string) => String(values[Number(index)] ?? ''));
}
export function message(key: MessageKey, ...values: unknown[]) { return t(key, ...values); }
export function number(value: number) { return new Intl.NumberFormat(language).format(value); }
export function date(value: string | Date) { return new Intl.DateTimeFormat(language, {dateStyle: 'medium', timeStyle: 'short'}).format(new Date(value)); }
export function plural(count: number, one: MessageKey, other: MessageKey) { return t(new Intl.PluralRules(language).select(count) === 'one' ? one : other); }
