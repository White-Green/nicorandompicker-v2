# Visual regression capture

The suite captures the empty state, restored result list, result hover/focus states, and open/closed player states at the viewports where each visual state is relevant. The tests seed `sessionStorage` with repository-managed `VideoContent` fixtures and intercept the restore API, thumbnails, and NicoNico player iframe. Any other external request is recorded as a test failure.

This is a visual regression suite, not an end-to-end API test. Live API contracts, rate limiting, and Turnstile behavior are outside its scope.

Run locally after installing Chromium:

```sh
pnpm exec playwright install chromium
pnpm test:vrt:capture
```

Screenshots are written to `vrt-results/`. Pull requests upload those images from the unprivileged capture workflow. The `workflow_run` report workflow compares them with the latest successful `main` capture and publishes the HTML diff as a GitHub Actions artifact. Visual differences are non-blocking.
