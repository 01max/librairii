import {t, currentLanguage} from './i18n';
import en from '../../internal/localization/locales/en.json';
import fr from '../../internal/localization/locales/fr.json';

const historicalKeys = new Map(Object.entries(fr).map(([key, value]) => [value, key]));
const metadataSummaryKey = 'Official metadata refreshed; {0} local stories matched.';
const statusKeys: Record<string, string> = {
    succeeded: 'Completed', partially_succeeded: 'Completed with exceptions',
    skipped: 'Skipped', conflicted: 'Conflicted', cancelled: 'Cancelled',
    interrupted: 'Interrupted', queued: 'Queued', pending: 'Queued', running: 'In progress',
    failed: 'The action could not be completed',
};

function outcome(text: string): {key: string; values: unknown[]} | undefined {
    const key = Object.hasOwn(en, text) ? text : historicalKeys.get(text);
    if (key) return {key, values: []};
    const matched = /^Official metadata refreshed; (\d+) local stories matched\.$/.exec(text);
    if (matched) return {key: metadataSummaryKey, values: [Number(matched[1])]};
    return undefined;
}

export function outcomeKey(text: string): string | undefined {
    return outcome(text)?.key;
}

export function outcomeText(text: string, status = 'failed'): string {
    const message = outcome(text);
    if (message) return t(message.key, ...message.values);
    if (currentLanguage() === 'en') return text;
    return t(statusKeys[status] ?? 'Action result unavailable');
}
