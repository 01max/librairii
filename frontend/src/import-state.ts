import {outcomeText} from './OutcomeText';
import {t} from './i18n';
import {operations} from '../wailsjs/go/models';

export type ImportNoticeTone = 'working' | 'success' | 'warning' | 'error' | 'neutral';

export type ImportNotice = {
    state: string;
    tone: ImportNoticeTone;
    title: string;
    message: string;
};

const invalidArchiveCodes = new Set([
    'invalid_container',
    'unsupported_format',
    'ambiguous_structure',
    'unsafe_path',
    'link_entry',
    'entry_limit',
    'expanded_size_limit',
    'compression_ratio_limit',
    'nested_archive',
    'metadata_limit',
    'artwork_limit',
    'missing_entry',
    'missing_asset',
    'invalid_uuid',
    'malformed_metadata',
]);

function plural(count: number, singular: string, multiple = `${singular}s`): string {
    return t(count === 1 ? singular : multiple);
}

export function operationIsActive(snapshot: operations.Snapshot | null): boolean {
    return snapshot?.status === 'queued' || snapshot?.status === 'running';
}

export function operationIsTerminal(snapshot: operations.Snapshot): boolean {
    return [
        'succeeded',
        'partially_succeeded',
        'failed',
        'cancelled',
        'interrupted',
    ].includes(snapshot.status);
}

export function describeImport(snapshot: operations.Snapshot): ImportNotice {
    if (operationIsActive(snapshot)) {
        const completed = Math.min(snapshot.completedItems, snapshot.totalItems);
        return {
            state: 'importing',
            tone: 'working',
            title: t("Importing {0} {1}", snapshot.totalItems, plural(snapshot.totalItems, 'story')),
            message: t("{0} of {1} {2} inspected.", completed, snapshot.totalItems, plural(snapshot.totalItems, 'archive')),
        };
    }

    const imported = snapshot.items.filter((item) => item.outcomeCode === 'imported');
    const duplicates = snapshot.items.filter((item) => item.outcomeCode === 'duplicate_checksum');
    const conflicts = snapshot.items.filter((item) => item.outcomeCode === 'uuid_conflict');
    const invalid = snapshot.items.filter((item) => invalidArchiveCodes.has(item.outcomeCode ?? ''));

    if (imported.length > 0) {
        const needsAttention = snapshot.items.length - imported.length;
        return {
            state: 'success',
            tone: needsAttention === 0 ? 'success' : 'warning',
            title: t("{0} {1} imported", imported.length, plural(imported.length, 'story')),
            message: needsAttention === 0
                ? t("Your local collection is up to date.")
                : t("{0} {1} also needs attention.", needsAttention, plural(needsAttention, 'file')),
        };
    }
    if (duplicates.length === snapshot.items.length && duplicates.length > 0) {
        return {
            state: 'duplicate-checksum',
            tone: 'neutral',
            title: t("Already in your library"),
            message: t("{0} identical {1} skipped safely.", duplicates.length, plural(duplicates.length, 'archive')),
        };
    }
    if (conflicts.length > 0) {
        return {
            state: 'uuid-conflict',
            tone: 'warning',
            title: t("A different archive uses this story UUID"),
            message: t("The existing story was kept and no managed bytes were replaced."),
        };
    }
    if (invalid.length > 0) {
        return {
            state: 'invalid-format',
            tone: 'warning',
            title: t("Unsupported or invalid story archive"),
            message: t("Choose a valid Lunii story pack in PK, ZIP, or 7z format."),
        };
    }
    if (snapshot.status === 'cancelled') {
        return {
            state: 'cancelled',
            tone: 'neutral',
            title: t("Import cancelled"),
            message: t("No incomplete story was added to the collection."),
        };
    }
    return {
        state: 'failed-import',
        tone: 'error',
        title: t("The import could not be completed"),
        message: (snapshot.errorMessage ? outcomeText(snapshot.errorMessage) : null) || t("Your existing collection and source files were left unchanged."),
    };
}
