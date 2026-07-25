import {existsSync} from 'node:fs';
import {copyFile, mkdtemp, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import pixelmatch from 'pixelmatch';
import {PNG} from 'pngjs';
import {chromium} from 'playwright';
import {build, preview} from 'vite';
import {installParityClock} from './parity-clock.mjs';

const frontendRoot = resolve(fileURLToPath(new URL('..', import.meta.url)));
const repositoryRoot = resolve(frontendRoot, '..');
const canonicalPrototype = resolve(
    repositoryRoot,
    'openspec/ui-prototypes/05-archive-shelves.html',
);
const outputDirectory = await mkdtemp(
    join(tmpdir(), 'librairii-frontend-visual-parity-'),
);
const acceptanceViewports = [
    {width: 1181, height: 900},
    {width: 1180, height: 900},
    {width: 850, height: 900},
    {width: 560, height: 900},
];
const pixelThreshold = 0.1;
const maximumMismatchRatio = 0.001;

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
    await copyFile(canonicalPrototype, join(outputDirectory, 'canonical.html'));
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
        throw new Error('visual parity preview did not bind a TCP port');
    }
    const origin = `http://127.0.0.1:${address.port}`;
    browser = await chromium.launch({
        executablePath: resolveChromeExecutable(),
        headless: true,
    });

    const comparisons = [];
    for (const viewport of acceptanceViewports) {
        comparisons.push(await compareViewport(browser, origin, viewport));
    }
    const report = {
        canonicalPrototype: 'openspec/ui-prototypes/05-archive-shelves.html',
        applicationFixture: '?fixture=parity',
        pixelThreshold,
        maximumMismatchRatio,
        comparisons,
        withinTolerance: comparisons.every(
            (comparison) => comparison.mismatchRatio <= maximumMismatchRatio,
        ),
    };
    process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
    if (!report.withinTolerance) {
        process.exitCode = 1;
    }
} finally {
    await browser?.close();
    previewServer?.httpServer.close();
    await rm(outputDirectory, {recursive: true, force: true});
}

async function compareViewport(browserInstance, origin, viewport) {
    const canonicalPage = await browserInstance.newPage({viewport});
    const applicationPage = await browserInstance.newPage({viewport});
    try {
        await Promise.all([
            installParityClock(canonicalPage),
            installParityClock(applicationPage),
        ]);
        await Promise.all([
            canonicalPage.goto(`${origin}/canonical.html`, {
                waitUntil: 'networkidle',
            }),
            applicationPage.goto(`${origin}/?fixture=parity`, {
                waitUntil: 'networkidle',
            }),
        ]);
        await applicationPage.getByRole('heading', {name: 'Weekend adventures'})
            .waitFor();
        await applicationPage.getByText('Mood tag').first().waitFor();
        await applicationPage.locator('.choice span').filter({hasText: /^18$/})
            .first().waitFor({state: 'attached'});
        await Promise.all([
            prepareScreenshot(canonicalPage, true),
            prepareScreenshot(applicationPage, false),
        ]);
        const [canonicalBuffer, applicationBuffer] = await Promise.all([
            canonicalPage.screenshot({animations: 'disabled'}),
            applicationPage.screenshot({animations: 'disabled'}),
        ]);
        const canonical = PNG.sync.read(canonicalBuffer);
        const application = PNG.sync.read(applicationBuffer);
        if (
            canonical.width !== application.width ||
            canonical.height !== application.height
        ) {
            throw new Error(
                `${viewport.width}x${viewport.height}: screenshot dimensions differ`,
            );
        }
        const mismatchedPixels = pixelmatch(
            canonical.data,
            application.data,
            null,
            canonical.width,
            canonical.height,
            {
                includeAA: false,
                threshold: pixelThreshold,
            },
        );
        return {
            viewport,
            comparedPixels: canonical.width * canonical.height,
            mismatchedPixels,
            mismatchRatio: Number(
                (mismatchedPixels / (canonical.width * canonical.height))
                    .toFixed(6),
            ),
        };
    } finally {
        await canonicalPage.close();
        await applicationPage.close();
    }
}

async function prepareScreenshot(page, canonical) {
    await page.evaluate(async (hideGalleryLink) => {
        await document.fonts.ready;
        if (hideGalleryLink) {
            document.querySelector('.back')?.remove();
        }
        const style = document.createElement('style');
        style.textContent = `
            *,
            *::before,
            *::after {
                animation: none !important;
                caret-color: transparent !important;
                transition: none !important;
            }
        `;
        document.head.append(style);
        window.scrollTo(0, 0);
    }, canonical);
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
