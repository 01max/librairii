import {existsSync} from 'node:fs';
import {mkdtemp, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import AxeBuilder from '@axe-core/playwright';
import {chromium} from 'playwright';
import {build, preview} from 'vite';

const frontendRoot = resolve(fileURLToPath(new URL('..', import.meta.url)));
const outputDirectory = await mkdtemp(
    join(tmpdir(), 'librairii-frontend-accessibility-'),
);
const acceptanceViewports = [
    {width: 1181, height: 900},
    {width: 560, height: 900},
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
        throw new Error('accessibility preview did not bind a TCP port');
    }
    browser = await chromium.launch({
        executablePath: resolveChromeExecutable(),
        headless: true,
    });

    const results = [];
    for (const viewport of acceptanceViewports) {
        const context = await browser.newContext({viewport});
        const page = await context.newPage();
        try {
            await page.emulateMedia({
                contrast: 'more',
                reducedMotion: 'reduce',
            });
            await page.goto(
                `http://127.0.0.1:${address.port}/?fixture=parity`,
                {waitUntil: 'networkidle'},
            );
            await page.getByRole('heading', {name: 'Weekend adventures'})
                .waitFor();
            const axe = await new AxeBuilder({page})
                .withTags([
                    'wcag2a',
                    'wcag2aa',
                    'wcag21a',
                    'wcag21aa',
                ])
                .analyze();
            const violations = axe.violations.map((violation) => ({
                id: violation.id,
                impact: violation.impact,
                help: violation.help,
                targets: violation.nodes.flatMap(
                    (node) => node.target.map(String),
                ),
            }));
            const contract = await readAccessibilityContract(page);
            if (violations.length > 0) {
                throw new Error(
                    `${viewport.width}x${viewport.height}: Axe violations ${
                        JSON.stringify(violations)
                    }`,
                );
            }
            assertAccessibilityContract(viewport, contract);
            results.push({
                viewport,
                axeViolations: violations.length,
                ...contract,
            });
        } finally {
            await context.close();
        }
    }
    process.stdout.write(`${JSON.stringify({acceptanceCases: results}, null, 2)}\n`);
} finally {
    await browser?.close();
    previewServer?.httpServer.close();
    await rm(outputDirectory, {recursive: true, force: true});
}

async function readAccessibilityContract(page) {
    return page.evaluate(() => {
        const story = document.querySelector('.story');
        const rootStyle = getComputedStyle(document.documentElement);
        const storyStyle = story ? getComputedStyle(story) : null;
        return {
            positiveTabIndexes: document.querySelectorAll('[tabindex]:not([tabindex="0"]):not([tabindex="-1"])').length,
            unlabeledTags: [...document.querySelectorAll('.tag')].filter(
                (tag) => tag.textContent?.trim() === '',
            ).length,
            unlabeledShelves: [...document.querySelectorAll('.saved-picker')].filter(
                (shelf) => shelf.textContent?.trim() === '',
            ).length,
            reducedMotion: {
                rootScrollBehavior: rootStyle.scrollBehavior,
                storyAnimationDuration: storyStyle?.animationDuration ?? '',
                storyTransitionDuration: storyStyle?.transitionDuration ?? '',
            },
        };
    });
}

function assertAccessibilityContract(viewport, contract) {
    if (contract.positiveTabIndexes !== 0) {
        throw new Error(
            `${viewport.width}px: positive tabindex changes logical tab order`,
        );
    }
    if (contract.unlabeledTags !== 0 || contract.unlabeledShelves !== 0) {
        throw new Error(`${viewport.width}px: color-only labels detected`);
    }
    if (
        contract.reducedMotion.rootScrollBehavior !== 'auto' ||
        contract.reducedMotion.storyTransitionDuration !== '0s'
    ) {
        throw new Error(
            `${viewport.width}px: reduced-motion contract is not active`,
        );
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
