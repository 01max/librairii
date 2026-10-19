import {t} from './i18n';
export function systemLabel(definition: {key: string; source: string; label: string}): string {
    if (definition.source === 'user') return definition.label;
    return t(definition.label);
}
