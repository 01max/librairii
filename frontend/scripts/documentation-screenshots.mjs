import {existsSync} from 'node:fs';
import {mkdir, mkdtemp, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {chromium} from 'playwright';
import {build, preview} from 'vite';
import {installParityClock} from './parity-clock.mjs';

const frontendRoot = resolve(fileURLToPath(new URL('..', import.meta.url)));
const assets = resolve(frontendRoot, '../docs/assets');
const output = await mkdtemp(join(tmpdir(), 'librairii-docs-'));
const candidates = process.env.LIBRAIRII_CHROME_PATH
    ? [process.env.LIBRAIRII_CHROME_PATH]
    : process.platform === 'darwin'
        ? ['/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
            '/Applications/Chromium.app/Contents/MacOS/Chromium']
        : ['/usr/bin/google-chrome', '/usr/bin/chromium', '/usr/bin/chromium-browser'];
let server;
let browser;
process.env.VITE_INCLUDE_FIXTURES = '1';
try {
    const executablePath = candidates.find((path) => existsSync(path));
    if (!executablePath) throw new Error('Set LIBRAIRII_CHROME_PATH to Chrome or Chromium');
    await build({root: frontendRoot, logLevel: 'error', build: {outDir: output, emptyOutDir: true}});
    server = await preview({root: frontendRoot, logLevel: 'error', build: {outDir: output},
        preview: {host: '127.0.0.1', port: 0, strictPort: false}});
    const address = server.httpServer.address();
    if (!address || typeof address === 'string') throw new Error('Preview did not bind a TCP port');
    const origin = `http://127.0.0.1:${address.port}`;
    browser = await chromium.launch({executablePath, headless: true});
    const context = await browser.newContext({viewport: {width: 1440, height: 1000},
        deviceScaleFactor: 1, locale: 'en-GB', timezoneId: 'UTC', reducedMotion: 'reduce'});
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', (error) => errors.push(error.message));
    await installParityClock(page);
    await mkdir(assets, {recursive: true});

    await page.goto(`${origin}/?fixture=parity`, {waitUntil: 'networkidle'});
    await page.getByRole('heading', {name: 'Weekend adventures', exact: true}).waitFor();
    await page.getByText('Mood tag').first().waitFor();
    await page.locator('.choice span').filter({hasText: /^18$/}).first().waitFor({state: 'attached'});
    await capture('library.png');
    await page.getByRole('button', {name: 'Open details', exact: true}).click();
    const details = page.getByRole('dialog');
    await details.getByRole('heading', {name: 'The Little Prince', exact: true}).waitFor();
    await capture('story-details.png', details);
    await page.keyboard.press('Escape');
    await details.waitFor({state: 'detached'});
    await page.getByRole('navigation', {name: 'Primary navigation'})
        .getByRole('button', {name: 'Tags', exact: true}).click();
    const tags = page.getByRole('dialog', {name: 'Manage tags', exact: true});
    await tags.getByLabel('Color for Age').waitFor();
    await capture('tag-manager.png', tags);

    await page.setViewportSize({width: 1181, height: 800});
    await page.goto(`${origin}/?fixture=curated`, {waitUntil: 'networkidle'});
    await page.getByRole('button', {name: 'Weekend Shelf 3', exact: true}).click();
    await page.getByRole('heading', {name: 'Weekend Shelf', exact: true}).waitFor();
    await page.getByRole('button', {name: 'List', exact: true}).click();
    await capture('curated-shelf.png');

    await page.goto(`${origin}/?fixture=curated&review=removals`, {waitUntil: 'networkidle'});
    await page.getByRole('button', {name: 'Weekend Shelf 3', exact: true}).click();
    const review = page.getByRole('dialog', {name: 'Review Shelf updates', exact: true});
    await review.getByRole('combobox', {name: 'Decision for The Secret Forest'}).waitFor();
    await capture('shelf-review.png', review);

    await page.setViewportSize({width: 1440, height: 1000});
    await page.goto(`${origin}/?fixture=settings`, {waitUntil: 'networkidle'});
    const settings = page.getByRole('dialog', {name: 'Settings', exact: true});
    await settings.getByRole('button', {name: 'Change location…', exact: true}).waitFor();
    await capture('settings.png', settings);
    if (errors.length) throw new Error(errors.join('\n'));

    async function capture(filename, target = page) {
        await page.evaluate(async () => {
            await document.fonts.ready;
            await Promise.all([...document.images].map((image) => image.decode().catch(() => {})));
        });
        await page.addStyleTag({content: '* {caret-color: transparent !important;}'});
        if (await page.evaluate(() => document.documentElement.scrollWidth > innerWidth)) {
            throw new Error(`${filename}: horizontal overflow`);
        }
        await target.screenshot({path: join(assets, filename), animations: 'disabled'});
        process.stdout.write(`Captured docs/assets/${filename}\n`);
    }
} finally {
    await browser?.close();
    await server?.close();
    await rm(output, {recursive: true, force: true});
}
