import {existsSync} from 'node:fs';
import {mkdir, mkdtemp, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {chromium} from 'playwright';
import {build, preview} from 'vite';
import {installParityClock} from './parity-clock.mjs';

const frontendRoot = resolve(fileURLToPath(new URL('..', import.meta.url)));
const outputDirectory = await mkdtemp(
    join(tmpdir(), 'librairii-frontend-responsive-'),
);
const acceptanceCases = [
    {
        name: 'above 1180px',
        viewport: {width: 1181, height: 900},
        expected: {
            appDisplay: 'grid',
            visiblePerRow: [6, 6],
            singleStoryVisible: true,
            collectionRowVisible: 6,
            filtersDisplay: 'block',
            railWidth: 72,
            railDirection: 'column',
            drawerLeft: 310,
            drawerColumnCount: 3,
            drawerTagsDisplay: 'block',
            syncDisplay: 'block',
        },
    },
    {
        name: 'at 1180px',
        viewport: {width: 1180, height: 900},
        expected: {
            appDisplay: 'grid',
            visiblePerRow: [5, 5],
            singleStoryVisible: true,
            collectionRowVisible: 6,
            filtersDisplay: 'block',
            railWidth: 72,
            railDirection: 'column',
            drawerLeft: 310,
            drawerColumnCount: 3,
            drawerTagsDisplay: 'block',
            syncDisplay: 'block',
        },
    },
    {
        name: 'at 850px',
        viewport: {width: 850, height: 900},
        expected: {
            appDisplay: 'grid',
            visiblePerRow: [4, 4],
            singleStoryVisible: true,
            collectionRowVisible: 6,
            filtersDisplay: 'none',
            railWidth: 64,
            railDirection: 'column',
            drawerLeft: 64,
            drawerColumnCount: 3,
            drawerTagsDisplay: 'block',
            syncDisplay: 'block',
        },
    },
    {
        name: 'at 560px',
        viewport: {width: 560, height: 900},
        expected: {
            appDisplay: 'grid',
            visiblePerRow: [3, 3],
            singleStoryVisible: true,
            collectionRowVisible: 6,
            filtersDisplay: 'none',
            railWidth: 560,
            railHeight: 58,
            railDirection: 'row',
            drawerLeft: 0,
            drawerColumnCount: 3,
            drawerTagsDisplay: 'block',
            syncDisplay: 'block',
            mainPadding: '22px 14px 215px',
            drawerArtworkHeight: 85,
            drawerFactsDisplay: 'none',
            drawerSubtitleDisplay: 'none',
            firstDrawerActionDisplay: 'block',
        },
    },
];

process.env.VITE_INCLUDE_FIXTURES = '1';
let previewServer;
let browser;
try {
    await build({
        root: frontendRoot,
        logLevel: 'error',
        build: {
            outDir: outputDirectory,
            emptyOutDir: true,
        },
    });
    previewServer = await preview({
        root: frontendRoot,
        logLevel: 'error',
        build: {outDir: outputDirectory},
        preview: {
            host: '127.0.0.1',
            port: 0,
            strictPort: false,
        },
    });
    const address = previewServer.httpServer.address();
    if (!address || typeof address === 'string') {
        throw new Error('responsive preview did not bind a TCP port');
    }
    browser = await chromium.launch({
        executablePath: resolveChromeExecutable(),
        headless: true,
    });

    const results = [];
    for (const acceptanceCase of acceptanceCases) {
        const page = await browser.newPage({viewport: acceptanceCase.viewport});
        try {
            await installParityClock(page);
            await installFixtureQueryRecorder(page);
            await page.goto(
                `http://127.0.0.1:${address.port}/?fixture=parity`,
                {waitUntil: 'networkidle'},
            );
            await page.getByRole('heading', {name: 'Weekend adventures'})
                .waitFor();
            await page.getByText('Mood tag').first().waitFor();
            await page.locator('.choice span').filter({hasText: /^18$/})
                .first().waitFor({state: 'attached'});
            const actual = await readResponsiveState(page);
            assertResponsiveState(
                acceptanceCase.name,
                actual,
                acceptanceCase.expected,
            );
            if (acceptanceCase.viewport.width <= 850) await assertSidebarAccess(page);
            await assertViewportScrolling(page);
            if (acceptanceCase.name === 'above 1180px') {
                await assertPopupInteractions(page);
                await assertRealAgeFacetAction(page);
                await assertFamilyBrowsingArtwork(page);
                await page.goto(`http://127.0.0.1:${address.port}/?fixture=parity`, {waitUntil: 'networkidle'});
                await assertLibraryViews(page);
            }
            await assertNoticeDismissal(page);
            await assertHistoryView(page);
            await assertFrenchControlsFit(page);
            results.push({
                name: acceptanceCase.name,
                viewport: acceptanceCase.viewport,
                actual,
            });
        } finally {
            await page.close();
        }
    }
    process.stdout.write(`${JSON.stringify({acceptanceCases: results}, null, 2)}\n`);
} finally {
    await browser?.close();
    previewServer?.httpServer.close();
    await rm(outputDirectory, {recursive: true, force: true});
}

async function assertNoticeDismissal(page) {
    const panel = page.getByRole('region', {name: 'Official Lunii sync'});
    const close = panel.getByRole('button', {name: 'Close Lunii sync'});
    await close.scrollIntoViewIfNeeded();
    await close.click();
    await panel.waitFor({state: 'detached'});
    const reopen = page.getByRole('button', {name: 'Show Lunii sync'});
    if (!await reopen.evaluate((button) => button === document.activeElement)) {
        throw new Error('closing the Lunii panel did not focus its disclosure');
    }
    const main = page.locator('.main');
    if (await main.evaluate((element) => element.scrollWidth > element.clientWidth)) {
        throw new Error('dismissed Lunii disclosure overflows the collection');
    }
    await reopen.press('Enter');
    await panel.waitFor();
    await panel.getByRole('button', {name: 'Sync with Lunii'}).waitFor();
}

async function assertHistoryView(page) {
    const url = new URL(page.url());
    url.search = '?fixture=history';
    url.hash = '';
    await page.goto(url.toString(), {waitUntil: 'networkidle'});
    await page.getByRole('button', {name: 'Close Export complete'}).click();
    await page.getByRole('button', {name: 'Close Lunii sync'}).click();
    await page.reload({waitUntil: 'networkidle'});
    await page.getByRole('button', {name: 'History', exact: true}).waitFor();
    if (await page.getByRole('heading', {name: 'Export complete'}).count() ||
        await page.getByRole('region', {name: 'Official Lunii sync'}).count()) {
        throw new Error('dismissed notices reopened after reloading the application');
    }
    const historyButton = page.getByRole('button', {name: 'History', exact: true});
    await historyButton.click();
    const dialog = page.getByRole('dialog', {name: 'History'});
    await dialog.getByText('Export', {exact: true}).click();
    const report = dialog.getByRole('region', {name: 'Export report'});
    await report.getByText('Milo and the Moon', {exact: true}).waitFor();
    await report.getByRole('button', {name: 'Reveal destination'}).click();
    await dialog.getByText('Import', {exact: true}).click();
    await dialog.getByText('Story imported.', {exact: true}).waitFor();
    const bounds = await dialog.boundingBox();
    const viewport = page.viewportSize();
    if (!bounds || bounds.x < 0 || bounds.y < 0 || bounds.x + bounds.width > viewport.width ||
        bounds.y + bounds.height > viewport.height ||
        await dialog.evaluate((element) => element.scrollWidth > element.clientWidth)) {
        throw new Error('History does not fit inside the window');
    }
    await page.screenshot({path: join(tmpdir(), `librairii-history-${viewport.width}.png`)});
    for (const height of [900, 320]) {
        await page.setViewportSize({width: 320, height});
        const compact = await dialog.boundingBox();
        if (!compact || compact.x < 0 || compact.y < 0 || compact.x + compact.width > 320 ||
            compact.y + compact.height > height ||
            await dialog.evaluate((element) => element.scrollWidth > element.clientWidth)) {
            throw new Error(`History overflows at 320x${height}`);
        }
    }
    await page.setViewportSize(viewport);
    await page.keyboard.press('Escape');
    await dialog.waitFor({state: 'detached'});
    if (!await historyButton.evaluate((button) => button === document.activeElement)) {
        throw new Error('History did not return keyboard focus to its opener');
    }
    await historyButton.click();
    await dialog.waitFor();
    await page.locator('.history-backdrop').click({position: {x: 2, y: 2}});
    await dialog.waitFor({state: 'detached'});
    // The sync panel can still be restored explicitly.
    await page.getByRole('button', {name: 'Show Lunii sync'}).click();
    await page.getByRole('region', {name: 'Official Lunii sync'}).waitFor();
}

async function assertSidebarAccess(page) {
    const viewport = page.viewportSize();
    const toggle = page.getByRole('button', {name: 'Library & filters', exact: true});
    if (!await toggle.isVisible()) {
        throw new Error(`${viewport.width}px: shelves and refinements are hidden with no way to reopen them`);
    }
    const panel = page.getByRole('dialog', {name: 'Saved collections and refinements'});
    const close = panel.getByRole('button', {name: 'Close collections and filters'});
    await toggle.click();
    await panel.waitFor();
    await page.waitForFunction(() => document.activeElement?.getAttribute('aria-label') === 'Close collections and filters');
    await page.keyboard.press('Shift+Tab');
    await page.waitForFunction(() => document.activeElement?.textContent.includes('Manage tags'));
    await page.keyboard.press('Tab');
    if (!await close.evaluate((button) => button === document.activeElement)) throw new Error('sidebar keyboard focus escaped');
    const bounds = await panel.boundingBox();
    if (!bounds || bounds.x < 0 || bounds.x + bounds.width > viewport.width || bounds.y + bounds.height > viewport.height) {
        throw new Error('sidebar does not fit inside the window');
    }
    if (viewport.width === 560 && viewport.height === 900) {
        await page.screenshot({path: join(tmpdir(), 'librairii-sidebar-review.png')});
    }
    // Exercise an actual query control without expanding the window.
    if (await panel.getByRole('combobox', {name: 'Broken stories'}).count()) throw new Error('retired Broken filter is still visible');
    const age = panel.getByRole('button', {name: 'Age', exact: true});
    await age.click();
    await age.click();
    await close.click();
    await panel.waitFor({state: 'hidden'});
    await page.waitForFunction(() => document.activeElement?.textContent === 'Library & filters');
    await toggle.click();
    await page.keyboard.press('Escape');
    await panel.waitFor({state: 'hidden'});
    await toggle.click();
    await page.locator('.sidebar-backdrop').click({position: {x: viewport.width - 2, y: 2}});
    await panel.waitFor({state: 'hidden'});
    await toggle.click();
    await panel.getByRole('button', {name: 'Manage tags'}).click();
    const manager = page.getByRole('dialog', {name: 'Manage tags'});
    await manager.waitFor();
    await page.keyboard.press('Escape');
    await manager.waitFor({state: 'detached'});
    if (!await panel.isVisible()) throw new Error('closing a nested dialog also closed the sidebar');
    await close.click();
    await panel.waitFor({state: 'hidden'});
    if (!await toggle.evaluate((button) => button === document.activeElement)) {
        throw new Error('closing the sidebar after a nested dialog did not restore disclosure focus');
    }
    // Opening a saved shelf keeps its management actions reachable.
    await toggle.click();
    await panel.getByRole('button', {name: 'Bedtime, 12 stories', exact: true}).click();
    await panel.getByRole('button', {name: 'Rename Bedtime'}).waitFor();
    await panel.getByRole('button', {name: 'All stories', exact: false}).click();
    await close.click();
}

async function assertLibraryViews(page) {
    await page.getByRole('heading', {name: 'Weekend adventures'}).waitFor();
    for (const width of [1181, 851, 850, 560, 320]) {
        await page.setViewportSize({width, height: 900});
        const picker = page.getByRole('group', {name: 'Story view'});
        const originalHash = new URL(page.url()).hash;
        const originalQueries = await page.evaluate(() => globalThis.__librairiiFixtureQueries.length);
        const originalSelection = await page.locator('.story[aria-pressed="true"]').count();
        const largeWidth = await page.locator('.story').first().evaluate((button) => button.getBoundingClientRect().width);
        for (const mode of ['Small covers', 'List', 'Large covers']) {
            await picker.getByRole('button', {name: mode, exact: true}).click();
            const layout = await page.evaluate(() => {
                const main = document.querySelector('.main');
                const story = document.querySelector('.story');
                return {overflow: main.scrollWidth > main.clientWidth,
                    width: story.getBoundingClientRect().width,
                    hasCover: !!story.querySelector('.cover'),
                    collectionRows: document.querySelectorAll('.story-row:not(.preview)').length,
                    previews: [...document.querySelectorAll('.story-row.preview')].map((row) =>
                        [...row.children].filter((card) => getComputedStyle(card).display !== 'none').length)};
            });
            if (layout.overflow) throw new Error(`${width}px ${mode}: collection overflows horizontally`);
            if (width === 1181 && mode !== 'Large covers') {
                await page.screenshot({path: join(tmpdir(), `librairii-${mode === 'List' ? 'list' : 'small-grid'}-review.png`)});
            }
            if (mode === 'Small covers' && layout.width >= largeWidth) throw new Error(`${width}px: small covers are not smaller`);
            if (mode === 'List' && layout.hasCover) throw new Error('list view still renders large artwork');
            if (mode !== 'Large covers' && layout.collectionRows !== 1) throw new Error('compact collection is still split into six-story shelves');
            if (mode !== 'Large covers' && layout.previews.some((count) => count !== 6)) throw new Error('compact view hides shelf preview stories');
            if (await picker.getByRole('button', {name: mode, exact: true}).getAttribute('aria-pressed') !== 'true') throw new Error('view is not announced');
            if (new URL(page.url()).hash !== originalHash || await page.locator('.story[aria-pressed="true"]').count() !== originalSelection ||
                await page.evaluate(() => globalThis.__librairiiFixtureQueries.length) !== originalQueries) throw new Error('changing views changed the collection');
        }
        if (width <= 850) {
            await page.setViewportSize({width, height: 320});
            await assertSidebarAccess(page);
            await page.getByRole('button', {name: 'Library & filters', exact: true}).click();
            await page.setViewportSize({width: 1181, height: 900});
            await page.getByRole('dialog', {name: 'Saved collections and refinements'}).waitFor({state: 'hidden'});
        }
    }
    await page.getByRole('button', {name: 'List', exact: true}).click();
    await page.reload({waitUntil: 'networkidle'});
    if (await page.getByRole('button', {name: 'List', exact: true}).getAttribute('aria-pressed') !== 'true') throw new Error('view preference did not survive reload');
}

async function installFixtureQueryRecorder(page) {
    await page.addInitScript(() => {
        globalThis.__librairiiFixtureQueries = [];
        globalThis.addEventListener('librairii:fixture-query', (event) => {
            globalThis.__librairiiFixtureQueries.push(event.detail);
        });
    });
}

async function assertPopupInteractions(page) {
    if (await page.getByRole('button', {name: 'Edit tags', exact: true}).count()) {
        throw new Error('Edit tags is still visible on the index');
    }
    await page.getByRole('button', {name: 'Open details', exact: true}).click();
    const details = page.getByRole('dialog');
    await details.getByRole('region', {name: 'Story tags'}).getByText('Age · 3–5', {exact: true}).waitFor();
    await details.getByRole('button', {name: 'Edit tags', exact: true}).click();
    const editor = page.getByRole('dialog', {name: 'Edit story tags'});
    await editor.getByRole('button', {name: 'Done', exact: true}).click();
    await details.getByRole('region', {name: 'Story tags'}).getByText('Age · 3–5', {exact: true}).waitFor();
    await details.getByRole('heading', {name: 'The Little Prince'}).click();
    if (!await details.isVisible()) throw new Error('inside click dismissed story details');
    await details.screenshot({path: join(tmpdir(), 'librairii-story-details-review.png')});
    await page.locator('.dialog-backdrop').click({position: {x: 5, y: 5}});
    await details.waitFor({state: 'detached'});
    await page.evaluate(() => {
        const load = window.go.main.App.StoryDetail;
        window.go.main.App.StoryDetail = async (...args) => {
            const result = await load(...args);
            result.detail.story.description = 'Once upon a time, our story begins.\n\n' + 'A long adventure unfolds.\n\n'.repeat(150);
            window.__longDescriptionLoaded = true;
            return result;
        };
    });
    await page.locator('.story').nth(1).click();
    await page.waitForFunction(() => window.__longDescriptionLoaded);
    for (let attempt = 0; attempt < 2; attempt++) {
        await page.getByRole('button', {name: 'Open details', exact: true}).click();
        await details.getByText(/^Once upon a time/).waitFor();
        await page.waitForFunction(() => document.activeElement?.textContent === 'Close');
        const scroll = await details.evaluate((dialog) => ({top: dialog.scrollTop, max: dialog.scrollHeight - dialog.clientHeight}));
        if (scroll.max <= 0 || scroll.top !== 0) throw new Error(`long story did not open at the beginning: ${JSON.stringify(scroll)}`);
        await details.evaluate((dialog) => { dialog.scrollTop = dialog.scrollHeight; });
        await details.getByRole('button', {name: 'Close', exact: true}).click();
    }
    await page.evaluate(() => {
        const app = window.go.main.App;
        const catalog = app.TagCatalog;
        const colors = {};
        app.TagCatalog = async () => {
            const response = await catalog();
            return {catalog: {definitions: response.catalog.definitions.map((definition) => ({
                ...definition, color: colors[definition.id] ?? definition.color,
            }))}};
        };
        app.RecolorTagDefinition = async (id, color) => {
            colors[id] = color;
            return {};
        };
    });
    const navigation = page.getByRole('navigation', {name: 'Primary navigation'});
    await navigation.getByRole('button', {name: 'Tags', exact: true}).click();
    const manager = page.getByRole('dialog', {name: 'Manage tags'});
    const color = manager.getByLabel('Color for Age');
    await color.waitFor();
    if (await color.inputValue() !== '#267d5d') throw new Error('age default is not green');
    await color.evaluate((input) => { input.value = '#405cf5'; });
    await manager.getByRole('button', {name: 'Save Age color'}).click();
    await manager.getByRole('button', {name: 'Save Age color'}).waitFor();
    await manager.screenshot({path: join(tmpdir(), 'librairii-tag-manager-review.png')});
    await page.locator('.tag-manager-backdrop').click({position: {x: 5, y: 5}});
    await manager.waitFor({state: 'detached'});
    await navigation.getByRole('button', {name: 'Tags', exact: true}).click();
    if (await manager.getByLabel('Color for Age').inputValue() !== '#405cf5') {
        throw new Error('protected tag color did not survive reopening');
    }
    await page.locator('.tag-manager-backdrop').click({position: {x: 5, y: 5}});
    await manager.waitFor({state: 'detached'});
    // Reset the fixture after verifying the custom color, before parity checks.
    await page.reload({waitUntil: 'networkidle'});
    await page.getByRole('heading', {name: 'Weekend adventures'}).waitFor();
}

async function assertViewportScrolling(page) {
    const initialViewport = page.viewportSize();
    for (const height of [900, 520, 320]) {
        await page.setViewportSize({...initialViewport, height});
        const layout = await page.evaluate(() => {
            const rail = document.querySelector('.rail');
            const main = document.querySelector('.main');
            return {
                windowHeight: innerHeight,
                documentHeight: document.documentElement.scrollHeight,
                railTop: rail.getBoundingClientRect().top,
                railHeight: rail.getBoundingClientRect().height,
                mainHeight: main.clientHeight,
                mainContentHeight: main.scrollHeight,
                avatar: document.querySelector('.avatar, [aria-label="Local profile"]') !== null,
            };
        });
        if (layout.documentHeight > height || layout.railTop !== 0 ||
            layout.railHeight > height || layout.avatar) {
            throw new Error(`viewport scroll regression at ${initialViewport.width}x${height}: ${JSON.stringify(layout)}`);
        }
        for (const action of await page.getByRole('navigation', {
            name: 'Primary navigation',
        }).getByRole('button').all()) {
            const bounds = await action.boundingBox();
            if (!bounds || bounds.y < 0 || bounds.y + bounds.height > height) {
                throw new Error(`navigation action is clipped at ${initialViewport.width}x${height}`);
            }
        }
        const main = page.locator('.main');
        await main.evaluate((element) => { element.scrollTop = 0; });
        await main.hover({position: {x: 10, y: 20}});
        await page.mouse.wheel(0, 500);
        await page.waitForFunction(() => document.querySelector('.main').scrollTop > 0);
        const after = await page.evaluate(() => ({
            windowScroll: window.scrollY,
            railTop: document.querySelector('.rail').getBoundingClientRect().top,
        }));
        if (after.windowScroll !== 0 || after.railTop !== 0) {
            throw new Error(`collection scrolling moved navigation: ${JSON.stringify(after)}`);
        }
        await page.getByRole('navigation', {name: 'Primary navigation'}).hover();
        await page.mouse.wheel(0, 500);
        if (await page.evaluate(() => window.scrollY !== 0 ||
            document.querySelector('.rail').getBoundingClientRect().top !== 0)) {
            throw new Error('wheel over navigation scrolls the application window');
        }
        await main.evaluate((element) => { element.scrollTop = 0; });
        const filters = page.locator('.filters');
        if (await filters.isVisible()) {
            await filters.evaluate((element) => {
                const extraFilters = document.createElement('div');
                extraFilters.dataset.scrollFixture = '';
                extraFilters.style.height = '1200px';
                element.append(extraFilters);
                element.scrollTop = 0;
            });
            await filters.hover({position: {x: 10, y: 20}});
            await page.mouse.wheel(0, 500);
            await page.waitForFunction(() => document.querySelector('.filters').scrollTop > 0);
            if (await page.evaluate(() => window.scrollY !== 0 ||
                document.querySelector('.rail').getBoundingClientRect().top !== 0)) {
                throw new Error('long filter list moves navigation or the window');
            }
            await filters.evaluate((element) => {
                element.querySelector('[data-scroll-fixture]').remove();
                element.scrollTop = 0;
            });
        }
    }
    await page.setViewportSize(initialViewport);
}

async function assertFamilyBrowsingArtwork(page) {
    // Exercise the real cover component with portrait, square and landscape
    // artwork. The prototype's decorative covers cannot catch image letterboxing.
    await page.route('**/artwork/ratio-*', (route) => {
        const storyID = Number(route.request().url().split('-').at(-1));
        const [width, height] = [[300, 400], [300, 300], [420, 300]][
            (storyID - 1) % 3
        ];
        return route.fulfill({
            contentType: 'image/svg+xml',
            body: `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}"><rect width="100%" height="100%" fill="#31559f"/></svg>`,
        });
    });
    await page.addInitScript(() => {
        let bridge;
        const enrich = (stories) => stories.map((story) => ({
            ...story, artworkId: `ratio-${story.id}`,
        }));
        Object.defineProperty(window, 'go', {
            configurable: true,
            get: () => bridge,
            set: (value) => {
                bridge = value;
                const app = value.main.App;
                const queryStories = app.QueryStories;
                const openShelf = app.OpenCollection;
                app.QueryStories = async (...args) => {
                    const response = await queryStories(...args);
                    response.page.stories = enrich(response.page.stories);
                    return response;
                };
                app.OpenCollection = async (...args) => {
                    const response = await openShelf(...args);
                    response.evaluation.page.stories = enrich(response.evaluation.page.stories);
                    return response;
                };
            },
        });
    });
    await page.reload({waitUntil: 'networkidle'});
    await page.locator('.story img').first().waitFor();
    for (const width of [1181, 560, 320]) {
        await page.setViewportSize({width, height: 900});
        for (const button of await page.locator('.story').all()) {
            if (!await button.isVisible()) {
                continue;
            }
            await button.scrollIntoViewIfNeeded();
            await button.locator('img').evaluate((img) => img.decode());
            const valid = await button.evaluate((element) => {
                const image = element.querySelector('img');
                const bounds = element.getBoundingClientRect();
                const imageBounds = image.getBoundingClientRect();
                return Math.abs(bounds.height - imageBounds.height) < 1 &&
                    Math.abs(bounds.width / bounds.height -
                        image.naturalWidth / image.naturalHeight) < .01 &&
                    !element.querySelector('b, small') &&
                    element.parentElement.querySelector('.story-title')?.textContent ===
                        element.getAttribute('aria-label');
            });
            if (!valid) {
                throw new Error(`${width}px: image button ratio or caption is incorrect`);
            }
        }
    }
    const url = new URL(page.url());
    url.search = '?fixture=first-sync';
    url.hash = '';
    await page.goto(url.toString(), {waitUntil: 'networkidle'});
    const sync = page.getByRole('region', {name: 'Official Lunii sync'});
    if (
        await sync.getAttribute('data-state') !== 'first-lunii-sync' ||
        !(await sync.innerText()).includes('Last successful sync: Never') ||
        !await sync.getByRole('button', {name: 'Sync with Lunii'}).isVisible()
    ) {
        throw new Error('narrow first-sync prompt is missing or unclear');
    }
    for (const action of await page.getByRole('navigation', {
        name: 'Primary navigation',
    }).getByRole('button').all()) {
        if (!await action.isVisible()) {
            throw new Error('a labeled rail action is hidden at 320px');
        }
    }
    url.search = '?fixture=broken-stories';
    await page.goto(url.toString(), {waitUntil: 'networkidle'});
    await page.setViewportSize({width: 1181, height: 900});
    if (await page.getByRole('combobox', {name: 'Broken stories'}).count()) throw new Error('retired Broken filter remains');
    const brokenStory = page.getByRole('button', {name: 'The Little Prince', exact: true});
    await brokenStory.waitFor();
    if (await brokenStory.locator('.broken-badge').count() !== 1) throw new Error('Broken badge missing');
    const artworkFilter = await brokenStory.locator('.cover').evaluate((cover) => getComputedStyle(cover).filter);
    if (!artworkFilter.includes('saturate(0.18)')) throw new Error('broken cover is not visually muted');
    for (const view of ['List', 'Small covers', 'Large covers']) {
        await page.getByRole('button', {name: view, exact: true}).click();
        await page.getByRole('button', {name: 'The Secret Forest', exact: true}).click();
        const action = page.getByRole('switch', {name: 'Broken: The Secret Forest', exact: true});
        await action.focus();
        if (await action.evaluate((button) => getComputedStyle(button).opacity) !== '1') throw new Error(`${view}: flag action not revealed by keyboard focus`);
        await action.press('Enter');
        await page.waitForFunction(() => document.querySelector('[role=switch][aria-checked=true]'));
        await page.getByRole('button', {name: 'The Secret Forest', exact: true}).waitFor();
        await page.getByRole('button', {name: 'Undo', exact: true}).click();
    }
    const touchPage = await page.context().browser().newPage({viewport: {width: 390, height: 844}, hasTouch: true, isMobile: true});
    try {
        await touchPage.goto(url.toString(), {waitUntil: 'networkidle'});
        await touchPage.getByRole('button', {name: 'The Secret Forest', exact: true}).tap();
        const action = touchPage.getByRole('switch', {name: 'Broken: The Secret Forest', exact: true});
        await action.waitFor();
        if (await action.evaluate((button) => getComputedStyle(button).opacity) !== '1') throw new Error('touch flag action requires hover');
        await action.tap();
        await touchPage.waitForFunction(() => document.querySelector('[role=switch][aria-checked=true]'));
        await touchPage.getByRole('button', {name: 'Undo', exact: true}).tap();
        await touchPage.getByRole('button', {name: 'The Secret Forest', exact: true}).waitFor();
    } finally { await touchPage.close(); }
}

async function assertRealAgeFacetAction(page) {
    const ageToggle = page.getByRole('button', {name: 'Age', exact: true});
    await ageToggle.click();
    if (await page.getByRole('checkbox', {name: '3–5 years'}).isVisible()) throw new Error('Age filter did not collapse');
    if (await ageToggle.getAttribute('aria-expanded') !== 'false') throw new Error('Age filter state was not announced');
    await ageToggle.press('Enter');
    const ageFacet = page.getByRole('checkbox', {name: '3–5 years'});
    if (!await ageFacet.isChecked()) {
        throw new Error('canonical Age facet is not seeded through the query');
    }
    const callsBeforeClick = await page.evaluate(
        () => globalThis.__librairiiFixtureQueries.length,
    );
    await ageFacet.uncheck();
    await page.waitForFunction((previousCalls) => {
        const calls = globalThis.__librairiiFixtureQueries;
        const latest = calls.at(-1);
        return calls.length > previousCalls &&
            latest.pageSize === 12 &&
            !latest.choiceFilters?.some(
                (filter) => filter.definitionId === 10,
            );
    }, callsBeforeClick);
    if (new URL(page.url()).hash.includes('choice=')) {
        throw new Error('canonical Age facet did not update collection history');
    }
}

async function readResponsiveState(page) {
    return page.evaluate(() => {
        const app = document.querySelector('.app');
        const filters = document.querySelector('.filters');
        const rail = document.querySelector('.rail');
        const main = document.querySelector('.main');
        const drawer = document.querySelector('.drawer');
        const drawerTags = document.querySelector('.drawer-tags');
        const drawerArtwork = document.querySelector('.drawer-art');
        const drawerFacts = document.querySelector('.facts');
        const drawerSubtitle = document.querySelector('.drawer-info > p');
        const firstDrawerAction = document.querySelector(
            '.drawer-actions button:first-child',
        );
        const sync = document.querySelector('.lunii-sync > button');
        if (
            !app || !filters || !rail || !main || !drawer || !drawerTags ||
            !drawerArtwork || !drawerFacts || !drawerSubtitle ||
            !firstDrawerAction || !sync
        ) {
            throw new Error('canonical responsive fixture is incomplete');
        }
        const singleRow = document.createElement('div');
        singleRow.className = 'story-row preview';
        const singleStory = document.querySelector('.story-row .story-card')?.cloneNode(true);
        if (!singleStory) {
            throw new Error('canonical responsive fixture has no story');
        }
        singleRow.append(singleStory);
        main.append(singleRow);
        const singleStoryVisible = getComputedStyle(singleStory).display !== 'none';
        singleRow.remove();
        const maxPreview = innerWidth <= 560 ? 3 : innerWidth <= 850 ? 4 : innerWidth <= 1180 ? 5 : 6;
        for (let count = 1; count <= 6; count++) {
            const preview = document.createElement('div');
            preview.className = 'story-row preview';
            for (let index = 0; index < count; index++) preview.append(singleStory.cloneNode(true));
            main.append(preview);
            const visible = [...preview.children].filter((card) => getComputedStyle(card).display !== 'none').length;
            preview.remove();
            if (visible !== Math.min(count, maxPreview)) throw new Error(`${innerWidth}px: ${count}-story shelf shows ${visible} covers`);
        }
        const chip = drawerTags.querySelector('.tag');
        if (!chip || Number.parseFloat(getComputedStyle(chip).fontSize) < 12) {
            throw new Error('selected story tags are missing or too small');
        }
        const tagsBounds = drawerTags.getBoundingClientRect();
        if (tagsBounds.width <= 0 || tagsBounds.bottom > innerHeight || tagsBounds.left < 0 || tagsBounds.right > innerWidth) {
            throw new Error('selected story tags are outside the window');
        }
        const collectionRow = document.createElement('div');
        collectionRow.className = 'story-row';
        for (const story of document.querySelectorAll('.story-row .story-card')) {
            collectionRow.append(story.cloneNode(true));
            if (collectionRow.children.length === 6) {
                break;
            }
        }
        main.append(collectionRow);
        const collectionRowVisible = [...collectionRow.children].filter(
            (story) => getComputedStyle(story).display !== 'none',
        ).length;
        collectionRow.remove();
        return {
            appDisplay: getComputedStyle(app).display,
            singleStoryVisible,
            collectionRowVisible,
            visiblePerRow: [...document.querySelectorAll('.story-row.preview')].map(
                (row) => [...row.querySelectorAll('.story-card')].filter(
                    (story) => getComputedStyle(story).display !== 'none',
                ).length,
            ),
            filtersDisplay: getComputedStyle(filters).display,
            railWidth: rail.getBoundingClientRect().width,
            railHeight: rail.getBoundingClientRect().height,
            railDirection: getComputedStyle(rail).flexDirection,
            drawerLeft: Number.parseFloat(getComputedStyle(drawer).left),
            drawerColumnCount: getComputedStyle(drawer).gridTemplateColumns
                .split(' ').length,
            drawerTagsDisplay: getComputedStyle(drawerTags).display,
            syncDisplay: getComputedStyle(sync).display,
            mainPadding: getComputedStyle(main).padding,
            drawerArtworkHeight: drawerArtwork.getBoundingClientRect().height,
            drawerFactsDisplay: getComputedStyle(drawerFacts).display,
            drawerSubtitleDisplay: getComputedStyle(drawerSubtitle).display,
            firstDrawerActionDisplay: getComputedStyle(firstDrawerAction).display,
        };
    });
}

function assertResponsiveState(name, actual, expected) {
    for (const [key, expectedValue] of Object.entries(expected)) {
        const actualValue = actual[key];
        if (JSON.stringify(actualValue) !== JSON.stringify(expectedValue)) {
            throw new Error(
                `${name}: ${key} was ${JSON.stringify(actualValue)}, want ${
                    JSON.stringify(expectedValue)
                }`,
            );
        }
    }
}

function resolveChromeExecutable() {
    const configured = process.env.LIBRAIRII_CHROME_PATH;
    const candidates = configured
        ? [configured]
        : process.platform === 'darwin'
            ? [
                '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
                '/Applications/Chromium.app/Contents/MacOS/Chromium',
            ]
            : [
                '/usr/bin/google-chrome',
                '/usr/bin/chromium',
                '/usr/bin/chromium-browser',
            ];
    const executable = candidates.find((candidate) => existsSync(candidate));
    if (!executable) {
        throw new Error(
            'Chrome or Chromium was not found; set LIBRAIRII_CHROME_PATH',
        );
    }
    return executable;
}

async function assertFrenchControlsFit(page) {
    const url = new URL(page.url());
    url.search = '?fixture=history&lang=fr';
    url.hash = '';
    await page.goto(url.toString(), {waitUntil: 'networkidle'});
    await page.getByRole('button', {name: 'Bibliothèque', exact: true}).waitFor();
    const overflowingLabels = await page.locator('.rail nav button span').evaluateAll((labels) => labels
        .filter((label) => {
            const parent = label.closest('button').getBoundingClientRect();
            const bounds = label.getBoundingClientRect();
            return bounds.left < parent.left - 1 || bounds.right > parent.right + 1;
        }).map((label) => label.textContent));
    const screenshotDirectory = process.env.LIBRAIRII_LAYOUT_SCREENSHOTS;
    if (screenshotDirectory) {
        await mkdir(screenshotDirectory, {recursive: true});
        await page.screenshot({path: join(screenshotDirectory, `library-${page.viewportSize().width}.png`)});
    }
    await page.locator('.rail nav button').nth(1).click();
    const editor = page.locator('.protected-color');
    await editor.waitFor();
    const colorWidth = await page.locator('.tag-manager input[type="color"]').evaluateAll((inputs) =>
        Math.max(...inputs.map((input) => input.getBoundingClientRect().width)));
    const buttonOverflows = await editor.locator('button').evaluate((button) =>
        button.scrollHeight > button.clientHeight || button.scrollWidth > button.clientWidth);
    if (screenshotDirectory) {
        await page.screenshot({path: join(screenshotDirectory, `tags-${page.viewportSize().width}.png`)});
    }
    if (overflowingLabels.length || colorWidth > 80 || buttonOverflows) {
        throw new Error(`French controls overflow: ${JSON.stringify({overflowingLabels, colorWidth, buttonOverflows})}`);
    }
}
