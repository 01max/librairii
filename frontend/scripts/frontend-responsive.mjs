import {existsSync} from 'node:fs';
import {mkdtemp, rm} from 'node:fs/promises';
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
            drawerColumnCount: 4,
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
            drawerTagsDisplay: 'none',
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
            drawerTagsDisplay: 'none',
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
            drawerTagsDisplay: 'none',
            syncDisplay: 'block',
            mainPadding: '22px 14px 215px',
            drawerArtworkHeight: 85,
            drawerFactsDisplay: 'none',
            drawerSubtitleDisplay: 'none',
            firstDrawerActionDisplay: 'none',
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
            await assertViewportScrolling(page);
            if (acceptanceCase.name === 'above 1180px') {
                await assertRealAgeFacetAction(page);
                await assertFamilyBrowsingArtwork(page);
            }
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

async function installFixtureQueryRecorder(page) {
    await page.addInitScript(() => {
        globalThis.__librairiiFixtureQueries = [];
        globalThis.addEventListener('librairii:fixture-query', (event) => {
            globalThis.__librairiiFixtureQueries.push(event.detail);
        });
    });
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
                const openShelf = app.OpenShelf;
                app.QueryStories = async (...args) => {
                    const response = await queryStories(...args);
                    response.page.stories = enrich(response.page.stories);
                    return response;
                };
                app.OpenShelf = async (...args) => {
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
}

async function assertRealAgeFacetAction(page) {
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
        singleRow.className = 'story-row';
        const singleStory = document.querySelector('.story-row .story-card')?.cloneNode(true);
        if (!singleStory) {
            throw new Error('canonical responsive fixture has no story');
        }
        singleRow.append(singleStory);
        main.append(singleRow);
        const singleStoryVisible = getComputedStyle(singleStory).display !== 'none';
        singleRow.remove();
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
            visiblePerRow: [...document.querySelectorAll('.story-row')].map(
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
