# Visual regression capture

Phase 1 captures the empty state, restored result list, and open player at desktop and mobile viewports. The tests seed `sessionStorage` with repository-managed `VideoContent` fixtures and intercept the restore API, thumbnails, and NicoNico player iframe. Any other external request is recorded as a test failure.

Run locally after installing Chromium:

```sh
pnpm exec playwright install chromium
pnpm test:vrt:capture
```

Screenshots are written to `vrt-results/`. Pull requests upload those images from the unprivileged capture workflow. The `workflow_run` report workflow compares them with the latest successful `main` capture and publishes the HTML diff as a GitHub Actions artifact. Visual differences are initially non-blocking.
