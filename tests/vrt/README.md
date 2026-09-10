# Visual regression capture

The suite captures the empty state, restored result list, result hover/focus states, and open/closed player states. It uses mobile and desktop viewports in both portrait and landscape orientations. Hover and touch focus are captured only on the device class where they are relevant.

The tests seed `sessionStorage` with repository-managed `VideoContent` fixtures and intercept the restore API, thumbnails, and NicoNico player iframe. Any other external request is recorded as a test failure.

This is a visual regression suite, not an end-to-end API test. Live API contracts, rate limiting, and Turnstile behavior are outside its scope.

Run locally after installing Chromium:

```sh
pnpm exec playwright install chromium
pnpm test:vrt:capture
```

Screenshots are written to `vrt-results/`. Pull requests upload those images from the unprivileged capture workflow. The `workflow_run` report workflow compares them with the baseline stored in Cloudflare R2, publishes the HTML diff to Cloudflare Pages, and links it from the pull request. Visual differences are non-blocking.

The report workflow reuses the repository's `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` secrets. It also requires `VRT_CF_R2_BUCKET` and `VRT_CF_PAGES_PROJECT_NAME` repository variables. The bucket and Pages project must exist before the report workflow runs.
