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
            appDisplay: 'block',
            visiblePerRow: [3, 3],
            filtersDisplay: 'none',
            railWidth: 560,
            railHeight: 58,
            railDirection: 'row',
            drawerLeft: 0,
            drawerColumnCount: 3,
            drawerTagsDisplay: 'none',
            syncDisplay: 'none',
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
            if (acceptanceCase.name === 'above 1180px') {
                await assertRealAgeFacetAction(page);
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
        const sync = document.querySelector('.top button:first-of-type');
        if (
            !app || !filters || !rail || !main || !drawer || !drawerTags ||
            !drawerArtwork || !drawerFacts || !drawerSubtitle ||
            !firstDrawerAction || !sync
        ) {
            throw new Error('canonical responsive fixture is incomplete');
        }
        return {
            appDisplay: getComputedStyle(app).display,
            visiblePerRow: [...document.querySelectorAll('.story-row')].map(
                (row) => [...row.querySelectorAll('.story')].filter(
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
            : process.platform === 'win32'
                ? [
                    'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',
                    'C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe',
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
