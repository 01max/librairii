import type {LibraryView} from './library-view';
import type {HandoverFrame} from './query-history';

export type PreferenceHandover = {
    presentation: 'list' | 'small_covers' | 'large_covers';
    dismissedOperations: string[];
    syncPanelHidden: boolean;
    frames: HandoverFrame[];
    position: number;
};
type TransferResponse = {success?: boolean; error?: {message: string}};
type TransferWindow = Window & {go?: {main?: {App?: {ExportNativePreferences?: (state: string) => Promise<TransferResponse>}}}};

export async function exportPreferenceHandover(view: LibraryView, dismissed: Set<string>, syncHidden: boolean,
    navigation: {frames: HandoverFrame[]; position: number}): Promise<boolean> {
    const state: PreferenceHandover = {
        presentation: view === 'small' ? 'small_covers' : view === 'large' ? 'large_covers' : 'list',
        dismissedOperations: [...dismissed], syncPanelHidden: syncHidden,
        frames: navigation.frames, position: navigation.position,
    };
    const exportPreferences = (window as TransferWindow).go?.main?.App?.ExportNativePreferences;
    if (!exportPreferences) throw new Error('Preference transfer could not be saved.');
    const response = await exportPreferences(JSON.stringify(state));
    if (response.error) throw new Error(response.error.message);
    return response.success === true;
}
