# Visual regression capture

The suite captures the empty state, restored result list, result hover/focus states, and open/closed player states. It uses mobile and desktop viewports in both portrait and landscape orientations. Hover and touch focus are captured only on the device class where they are relevant.

The tests seed `sessionStorage` with repository-managed `VideoContent` fixtures and intercept the restore API, thumbnails, and NicoNico player iframe. Any other external request is recorded as a test failure.

This is a visual regression suite, not an end-to-end API test. Live API contracts, rate limiting, and Turnstile behavior are outside its scope.

Run locally after installing Chromium:

```sh
pnpm exec playwright install chromium
pnpm test:vrt:capture
```

Screenshots are written to `vrt-results/`. Pull requests upload those images from the unprivileged capture workflow. The `workflow_run` report workflow compares them with the baseline stored in Cloudflare R2, uploads the HTML diff as static assets in a Cloudflare Worker version, and links its PR-specific preview URL from the pull request. Visual differences are non-blocking.

The report workflow reuses the repository's `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` secrets and requires the `VRT_CF_R2_BUCKET` repository variable. The API token needs permission to read/write R2 objects and edit Workers scripts, and the Cloudflare account must have a `workers.dev` subdomain. The R2 bucket must exist before the workflow runs; the workflow creates or updates the `nicorandompicker-v2-vrt` Worker with public preview URLs enabled and no production `workers.dev` route, so no Pages project or pre-created Worker is required.

Main-branch baselines use immutable, extensionless R2 object keys of the form `visual-regression/nicorandompicker-v2/baselines/<commit-sha>`. A pull request reads the baseline for its base commit. This prevents concurrent main runs from overwriting one another and avoids [Wrangler's known stale reads for overwritten `.zst` keys](https://github.com/cloudflare/developer-platform/issues/53). An R2 lifecycle rule may remove old baseline objects after a retention period long enough for the repository's usual pull-request lifetime.

Renovate custom managers track the report workflow's wrangler and semdiff versions. Wrangler's workflow pin is grouped with the `package.json` dependency so both references are updated together.
