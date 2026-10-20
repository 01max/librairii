import {existsSync} from 'node:fs';
import {mkdtemp, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import AxeBuilder from '@axe-core/playwright';
import {chromium} from 'playwright';
import {build, preview} from 'vite';
import {installParityClock} from './parity-clock.mjs';

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
            await installParityClock(page);
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
            await page.getByText('Mood tag').first().waitFor();
            await page.locator('.choice span').filter({hasText: /^18$/})
                .first().waitFor({state: 'attached'});
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
            await page.getByRole('button', {name: 'Filter by tags', exact: true}).click();
            await page.getByRole('region', {name: 'Tag filters', exact: true}).waitFor();
            const tagFiltersResult = await new AxeBuilder({page}).withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa']).analyze();
            if (tagFiltersResult.violations.length) throw new Error(`tag search: ${JSON.stringify(tagFiltersResult.violations)}`);
            await page.keyboard.press('Escape');
            if (!await page.getByRole('button', {name: 'Filter by tags', exact: true}).evaluate((button) => button === document.activeElement)) throw new Error('tag search did not restore focus');
            await page.getByRole('button', {name: 'Open details', exact: true}).click();
            const storyDetails = page.getByRole('dialog', {name: 'The Little Prince', exact: true});
            await storyDetails.waitFor();
            if (await storyDetails.locator('.flag-feedback').count()) throw new Error('Broken notice appeared before a change');
            await storyDetails.getByRole('switch', {name: 'Broken: The Little Prince', exact: true}).click();
            await storyDetails.getByRole('button', {name: 'Undo', exact: true}).waitFor();
            const toggleResult = await new AxeBuilder({page}).withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa']).analyze();
            if (toggleResult.violations.length) throw new Error(`Broken toggle: ${JSON.stringify(toggleResult.violations)}`);
            await storyDetails.getByRole('button', {name: 'Undo', exact: true}).click();
            await storyDetails.getByRole('button', {name: 'Close', exact: true}).click();
            for (const view of ['List', 'Small covers']) {
                await page.getByRole('group', {name: 'Story view'}).getByRole('button', {name: view, exact: true}).click();
                const result = await new AxeBuilder({page}).withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa']).analyze();
                if (result.violations.length) throw new Error(`${viewport.width}px ${view}: ${JSON.stringify(result.violations)}`);
            }
            if (viewport.width <= 850) {
                await page.getByRole('button', {name: 'Library & filters', exact: true}).click();
                await page.getByRole('dialog', {name: 'Saved collections and refinements'}).waitFor();
                const result = await new AxeBuilder({page}).withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa']).analyze();
                if (result.violations.length) throw new Error(`sidebar: ${JSON.stringify(result.violations)}`);
            }
            await page.goto(`http://127.0.0.1:${address.port}/?fixture=history`, {waitUntil: 'networkidle'});
            await page.getByRole('button', {name: 'History', exact: true}).click();
            const history = page.getByRole('dialog', {name: 'History'});
            await history.getByText('Export', {exact: true}).click();
            await history.getByText('Milo and the Moon', {exact: true}).waitFor();
            const historyResult = await new AxeBuilder({page})
                .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa']).analyze();
            if (historyResult.violations.length) throw new Error(`History: ${JSON.stringify(historyResult.violations)}`);
            for (const fixture of ['settings', 'setup', 'relocating']) {
                await page.goto(`http://127.0.0.1:${address.port}/?fixture=${fixture}&lang=fr`, {waitUntil:'networkidle'});
                await page.getByRole('dialog').waitFor();
                const settingsResult = await new AxeBuilder({page}).withTags(['wcag2a','wcag2aa','wcag21a','wcag21aa']).analyze();
                if (settingsResult.violations.length) throw new Error(`${fixture}: ${JSON.stringify(settingsResult.violations)}`);
                if (await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth)) throw new Error(`${fixture}: horizontal overflow`);
            }
            await assertCuration(page, `http://127.0.0.1:${address.port}`, viewport);
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


async function assertCuration(page, origin, viewport) {
    await page.goto(`${origin}/?fixture=curated`, {waitUntil: 'networkidle'});
    if (viewport.width <= 850) await page.getByRole('button', {name: 'Library & filters', exact: true}).click();
    await page.getByRole('button', {name: 'Weekend Shelf 3', exact: true}).click();
    await page.getByRole('heading', {name: 'Weekend Shelf', exact: true}).waitFor();
    const check = async (name) => {
        const result = await new AxeBuilder({page}).withTags(['wcag2a','wcag2aa','wcag21a','wcag21aa']).analyze();
        if (result.violations.length) throw new Error(`${viewport.width}px ${name}: ${JSON.stringify(result.violations)}`);
        if (await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth)) {await page.screenshot({path: join(tmpdir(), 'librairii-overflow.png')}); throw new Error(`${viewport.width}px ${name}: horizontal overflow ${await page.evaluate(() => document.documentElement.scrollWidth)} ${JSON.stringify(await page.evaluate(() => [...document.querySelectorAll('.app *')].filter((e) => e.getBoundingClientRect().right > innerWidth + 1 && getComputedStyle(e).display !== 'none').slice(0, 8).map((e) => ({tag:e.tagName,cls:e.className,right:e.getBoundingClientRect().right,text:e.textContent.slice(0,60)}))))}`);}
    };
    await check('curated Shelf');
    await page.screenshot({path: join(tmpdir(), `librairii-curated-${viewport.width}.png`)});
    const reviewButton = page.getByRole('button', {name: 'Review updates', exact: true});
    await reviewButton.click();
    const review = page.getByRole('dialog', {name: 'Review Shelf updates'});
    await review.getByRole('combobox').first().waitFor();
    if (await review.getByRole('combobox').count() !== 25) throw new Error('review rendered more than one candidate page');
    if (!await review.getByRole('button', {name: 'Apply 0 approved changes'}).isDisabled()) throw new Error('review preapproved candidates');
    await check('update review');
    await page.screenshot({path: join(tmpdir(), `librairii-review-${viewport.width}.png`)});
    await page.keyboard.press('Escape');
    await review.waitFor({state:'detached'});
    if (!await reviewButton.evaluate((button) => button === document.activeElement)) throw new Error('review did not return focus');
    await page.getByRole('button', {name: 'Manage sources', exact:true}).click();
    await page.getByRole('button', {name: 'Preview source changes'}).click();
    await page.getByRole('combobox', {name: 'Source story'}).waitFor();
    await check('source detachment');
    await page.keyboard.press('Escape');
    await page.locator('.curated-management summary').click();
    await page.getByRole('button', {name: 'Manage exclusions (1)'}).click();
    await check('exclusions');
    await page.keyboard.press('Escape');
    if (viewport.width <= 850) await page.getByRole('button', {name:'Library & filters', exact:true}).click();
    await page.getByRole('button', {name:'Create Shelf', exact:true}).click();
    await page.getByRole('textbox', {name:'Shelf name'}).fill('New Shelf');
    await page.getByRole('button', {name:'Preview initial stories'}).click();
    await page.getByRole('button', {name:'Create with 0 stories'}).waitFor();
    await check('creation confirmation');
    await page.keyboard.press('Escape');
}
