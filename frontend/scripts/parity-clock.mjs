const parityTimestamp = Date.parse('2026-07-25T12:00:00.000Z');

export async function installParityClock(page) {
    await page.addInitScript((timestamp) => {
        const NativeDate = globalThis.Date;

        class FrozenDate extends NativeDate {
            constructor(...args) {
                super(...(args.length === 0 ? [timestamp] : args));
            }

            static now() {
                return timestamp;
            }
        }

        globalThis.Date = FrozenDate;
    }, parityTimestamp);
}
