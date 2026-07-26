import {existsSync} from 'node:fs';
import {mkdtemp, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {chromium} from 'playwright';
import {build, preview} from 'vite';
import performanceConfig from '../performance-config.json' with {type: 'json'};

const frontendRoot = resolve(fileURLToPath(new URL('..', import.meta.url)));
const storyCount = performanceConfig.browserStoryCount;
const sampleCount = Number.parseInt(
    process.env.LIBRAIRII_FRONTEND_PERFORMANCE_SAMPLES ?? '5',
    10,
);
const acceptanceBudgets = {
    expansionP95Milliseconds: 3_000,
    inputDelayP95Milliseconds: 100,
};
const diagnosticThresholds = {
    frameGapP95Milliseconds: 50,
    timerDelayP95Milliseconds: 50,
};

if (!Number.isInteger(sampleCount) || sampleCount < 1) {
    throw new Error('LIBRAIRII_FRONTEND_PERFORMANCE_SAMPLES must be positive');
}
if (!Number.isInteger(storyCount) || storyCount < 1) {
    throw new Error('browserStoryCount must be a positive integer');
}

const outputDirectory = await mkdtemp(
    join(tmpdir(), 'librairii-frontend-performance-'),
);
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
        build: {
            outDir: outputDirectory,
        },
        preview: {
            host: '127.0.0.1',
            port: 0,
            strictPort: false,
        },
    });
    const address = previewServer.httpServer.address();
    if (!address || typeof address === 'string') {
        throw new Error('frontend performance preview did not bind a TCP port');
    }
    const executablePath = resolveChromeExecutable();
    browser = await chromium.launch({
        executablePath,
        headless: true,
        args: [
            '--disable-background-timer-throttling',
            '--disable-renderer-backgrounding',
        ],
    });

    const samples = [];
    for (let sample = 0; sample < sampleCount; sample += 1) {
        samples.push(await measureSample(
            browser,
            `http://127.0.0.1:${address.port}/?fixture=performance`,
        ));
    }
    const report = summarize(samples);
    process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
    if (!report.withinBudget) {
        process.exitCode = 1;
    }
} finally {
    await browser?.close();
    previewServer?.httpServer.close();
    await rm(outputDirectory, {recursive: true, force: true});
}

async function measureSample(browserInstance, url) {
    const page = await browserInstance.newPage({
        viewport: {width: 1440, height: 900},
    });
    const browserErrors = [];
    page.on('pageerror', (error) => browserErrors.push(error.message));
    page.on('console', (message) => {
        if (message.type() === 'error') {
            browserErrors.push(message.text());
        }
    });
    try {
        await page.goto(url, {waitUntil: 'networkidle'});
        const viewAll = page
            .locator('section.shelf')
            .first()
            .locator('.shelf-head button');
        const collectionRail = page.getByRole('button', {
            name: 'Story collection',
            exact: true,
        });
        try {
            await viewAll.waitFor({state: 'visible', timeout: 5_000});
        } catch (error) {
            const body = await page.locator('body').innerText();
            throw new Error(
                `performance fixture did not become ready: ${browserErrors.join(
                    ' | ',
                )}; body=${body.slice(0, 1_000)}`,
                {cause: error},
            );
        }
        await page.evaluate(() => {
            const probe = {
                animationFrameGaps: [],
                inputDelays: [],
                timerDelays: [],
                longAnimationFrames: [],
                running: true,
                lastFrame: performance.now(),
                nextTimer: performance.now() + 25,
            };
            window.__librairiiPerformanceProbe = probe;
            const frame = (now) => {
                if (!probe.running) {
                    return;
                }
                probe.animationFrameGaps.push(now - probe.lastFrame);
                probe.lastFrame = now;
                window.requestAnimationFrame(frame);
            };
            window.requestAnimationFrame(frame);
            probe.timer = window.setInterval(() => {
                const now = performance.now();
                probe.timerDelays.push(Math.max(0, now - probe.nextTimer));
                probe.nextTimer = now + 25;
            }, 25);
            document.addEventListener('pointerdown', (event) => {
                probe.inputDelays.push(
                    Math.max(0, performance.now() - event.timeStamp),
                );
            }, {capture: true});
            if (PerformanceObserver.supportedEntryTypes.includes(
                'long-animation-frame',
            )) {
                probe.observer = new PerformanceObserver((list) => {
                    for (const entry of list.getEntries()) {
                        probe.longAnimationFrames.push(entry.duration);
                    }
                });
                probe.observer.observe({type: 'long-animation-frame'});
            }
        });

        const startedAt = await page.evaluate(() => performance.now());
        await viewAll.click();
        const railBox = await collectionRail.boundingBox();
        if (!railBox) {
            throw new Error('story collection rail button has no layout box');
        }
        const completionDeadline = Date.now() + 10_000;
        while ((await viewAll.textContent())?.trim() !== 'All shown') {
            if (Date.now() >= completionDeadline) {
                throw new Error(
                    `${storyCount.toLocaleString('en-US')}-story expansion exceeded 10 seconds`,
                );
            }
            await page.mouse.click(
                railBox.x + railBox.width / 2,
                railBox.y + railBox.height / 2,
            );
            await new Promise((resolveDelay) => setTimeout(resolveDelay, 25));
        }
        await page.evaluate(() => new Promise((resolveFrame) => {
            window.requestAnimationFrame(() => {
                window.requestAnimationFrame(resolveFrame);
            });
        }));
        const finishedAt = await page.evaluate(() => performance.now());
        const measurements = await page.evaluate(() => {
            const probe = window.__librairiiPerformanceProbe;
            probe.running = false;
            window.clearInterval(probe.timer);
            probe.observer?.disconnect();
            return {
                animationFrameGaps: probe.animationFrameGaps,
                inputDelays: probe.inputDelays,
                timerDelays: probe.timerDelays,
                longAnimationFrames: probe.longAnimationFrames,
                storyNodes: document.querySelectorAll('button.story').length,
            };
        });
        if (measurements.storyNodes !== storyCount) {
            throw new Error(
                `rendered ${measurements.storyNodes} story nodes, want ${storyCount}`,
            );
        }
        return {
            expansionMilliseconds: finishedAt - startedAt,
            ...measurements,
        };
    } finally {
        await page.close();
    }
}

function summarize(samples) {
    const expansionP95Milliseconds = percentile(
        samples.map((sample) => sample.expansionMilliseconds),
        0.95,
    );
    const frameGapP95Milliseconds = percentile(
        samples.flatMap((sample) => sample.animationFrameGaps),
        0.95,
    );
    const inputDelayP95Milliseconds = percentile(
        samples.flatMap((sample) => sample.inputDelays),
        0.95,
    );
    const timerDelayP95Milliseconds = percentile(
        samples.flatMap((sample) => sample.timerDelays),
        0.95,
    );
    const longAnimationFrameP95Milliseconds = percentile(
        samples.flatMap((sample) => sample.longAnimationFrames),
        0.95,
    );
    const metrics = {
        expansionP95Milliseconds,
        frameGapP95Milliseconds,
        inputDelayP95Milliseconds,
        timerDelayP95Milliseconds,
        longAnimationFrameP95Milliseconds,
    };
    return {
        generatedAt: new Date().toISOString(),
        browser: 'system Chromium via Playwright',
        viewport: {width: 1440, height: 900},
        stories: storyCount,
        samples: samples.length,
        acceptanceBudgets,
        diagnosticThresholds,
        metrics,
        maximumStoryNodes: Math.max(
            ...samples.map((sample) => sample.storyNodes),
        ),
        withinBudget:
            expansionP95Milliseconds <=
                acceptanceBudgets.expansionP95Milliseconds &&
            inputDelayP95Milliseconds <=
                acceptanceBudgets.inputDelayP95Milliseconds,
    };
}

function percentile(values, percentileValue) {
    if (values.length === 0) {
        return 0;
    }
    const ordered = [...values].sort((left, right) => left - right);
    const index = Math.max(
        0,
        Math.ceil(percentileValue * ordered.length) - 1,
    );
    return Number(ordered[index].toFixed(3));
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
