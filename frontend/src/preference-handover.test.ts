import {afterEach, expect, test, vi} from 'vitest';
import {exportPreferenceHandover} from './preference-handover';

afterEach(() => {Reflect.deleteProperty(window, 'go');});

test('sends only explicit semantic preferences to the native-folder export', async () => {
    const exportPreferences = vi.fn().mockResolvedValue({success: true});
    Object.assign(window, {go: {main: {App: {ExportNativePreferences: exportPreferences}}}});
    window.localStorage.setItem('unrelated-secret', 'never exported');
    expect(await exportPreferenceHandover('small', new Set(['operation']), true,
        {frames: [{hash: '#/library?v=3', kind: 'collection', id: 7}], position: 0})).toBe(true);
    expect(JSON.parse(exportPreferences.mock.calls[0][0])).toEqual({
        presentation: 'small_covers', dismissedOperations: ['operation'], syncPanelHidden: true,
        frames: [{hash: '#/library?v=3', kind: 'collection', id: 7}], position: 0,
    });
    exportPreferences.mockResolvedValue({});
    expect(await exportPreferenceHandover('list', new Set(), false, {frames: [], position: 0})).toBe(false);
    exportPreferences.mockResolvedValue({error: {message: 'preserved collision'}});
    await expect(exportPreferenceHandover('large', new Set(), false, {frames: [], position: 0})).rejects.toThrow('preserved collision');
});
