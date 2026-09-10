import { expect, test, type Page, type Route } from "@playwright/test";
import { mkdir } from "node:fs/promises";
import path from "node:path";
import {
  appStateStorageKey,
  type PersistedState,
} from "../../frontend/src/storage";
import { videoContents } from "./fixtures/video-contents";

const emptyState: PersistedState = {
  version: 1,
  search: {
    tag: "",
    uploadedSince: null,
    uploadedUntil: null,
    viewMin: null,
    viewMax: null,
    resultCount: 10,
  },
  results: { contentIds: [], selectedVideo: null },
  player: { enabled: false, loopType: "Loop" },
};

const restoredState: PersistedState = {
  version: 1,
  search: {
    tag: "固定fixture VRT",
    uploadedSince: "2025-01-01T00:00",
    uploadedUntil: "2025-12-31T23:59",
    viewMin: 1_000,
    viewMax: 1_000_000,
    resultCount: 10,
  },
  results: {
    contentIds: videoContents.map(({ contentId }) => contentId),
    selectedVideo: null,
  },
  player: { enabled: false, loopType: "Loop" },
};

const playerState: PersistedState = {
  ...restoredState,
  results: {
    ...restoredState.results,
    selectedVideo: {
      contentId: videoContents[0].contentId,
      tags: videoContents[0].tags,
    },
  },
  player: { enabled: true, loopType: "Loop" },
};

const playerClosedState: PersistedState = {
  ...playerState,
  player: { ...playerState.player, enabled: false },
};

const thumbnails: Record<string, { from: string; to: string; label: string }> =
  {
    spring: { from: "#166534", to: "#86efac", label: "SPRING" },
    rain: { from: "#1e3a8a", to: "#7dd3fc", label: "RAIN" },
    night: { from: "#312e81", to: "#c4b5fd", label: "NIGHT" },
    dawn: { from: "#9a3412", to: "#fdba74", label: "DAWN" },
  };

test.describe("visual states", () => {
  test("empty", async ({ page }, testInfo) => {
    const unexpectedRequests = await preparePage(page, emptyState);
    await page.goto("/");
    await expect(page.getByLabel("タグ")).toBeVisible();
    await capture(page, testInfo.project.name, "empty");
    expect(unexpectedRequests).toEqual([]);
  });

  test("restored results", async ({ page }, testInfo) => {
    const unexpectedRequests = await preparePage(page, restoredState);
    await page.goto("/");
    await expect(page.locator('[id^="video-"]')).toHaveCount(
      videoContents.length,
    );
    await waitForImages(page);
    await capture(page, testInfo.project.name, "restored-results");
    expect(unexpectedRequests).toEqual([]);
  });

  test("result hover", async ({ page }, testInfo) => {
    test.skip(
      testInfo.project.name !== "desktop",
      "Hover is a desktop-only visual state",
    );
    const unexpectedRequests = await preparePage(page, restoredState);
    await page.goto("/");
    const firstResult = page.locator('[id^="video-"]').first();
    await expect(firstResult).toBeVisible();
    await waitForImages(page);
    await firstResult.hover();
    await capture(page, testInfo.project.name, "result-hover");
    expect(unexpectedRequests).toEqual([]);
  });

  test("result focus", async ({ page }, testInfo) => {
    test.skip(
      testInfo.project.name !== "mobile",
      "Touch focus is a mobile-only visual state",
    );
    const unexpectedRequests = await preparePage(page, restoredState);
    await page.goto("/");
    const firstResultContent = page
      .locator('[id^="video-"] .video_link_content')
      .first();
    await expect(firstResultContent).toBeVisible();
    await waitForImages(page);
    await firstResultContent.focus();
    await expect(firstResultContent).toBeFocused();
    await capture(page, testInfo.project.name, "result-focus");
    expect(unexpectedRequests).toEqual([]);
  });

  test("player open", async ({ page }, testInfo) => {
    const unexpectedRequests = await preparePage(page, playerState);
    await page.goto("/");
    await expect(page.locator('[id^="video-"]')).toHaveCount(
      videoContents.length,
    );
    await expect(page.getByTitle("nicovideo_player")).toBeVisible();
    await expect(
      page.frameLocator('[title="nicovideo_player"]').getByText("MOCK PLAYER"),
    ).toBeVisible();
    await waitForImages(page);
    await capture(page, testInfo.project.name, "player-open");
    expect(unexpectedRequests).toEqual([]);
  });

  test("player closed", async ({ page }, testInfo) => {
    const unexpectedRequests = await preparePage(page, playerClosedState);
    await page.goto("/");
    await expect(page.locator('[id^="video-"]')).toHaveCount(
      videoContents.length,
    );
    await expect(page.getByLabel("再生中の動画に戻る")).toBeVisible();
    await expect(page.getByTitle("nicovideo_player")).not.toBeVisible();
    await waitForImages(page);
    await capture(page, testInfo.project.name, "player-closed");
    expect(unexpectedRequests).toEqual([]);
  });
});

async function preparePage(
  page: Page,
  state: PersistedState,
): Promise<string[]> {
  const unexpectedRequests: string[] = [];

  await page.addInitScript(
    ({ key, value }) => sessionStorage.setItem(key, JSON.stringify(value)),
    { key: appStateStorageKey, value: state },
  );

  await page.route("**/*", async (route) => {
    const url = new URL(route.request().url());

    if (url.origin === "http://127.0.0.1:4173") {
      if (url.pathname === "/api/restore_video_details") {
        await fulfillRestoreRequest(route);
      } else if (url.pathname.startsWith("/api/")) {
        unexpectedRequests.push(`${route.request().method()} ${url.pathname}`);
        await route.fulfill({
          status: 500,
          body: "Unexpected VRT API request",
        });
      } else {
        await route.continue();
      }
      return;
    }

    if (url.origin === "https://vrt.invalid") {
      await fulfillThumbnail(route, url.pathname);
      return;
    }

    if (url.origin === "https://embed.nicovideo.jp") {
      await fulfillPlayer(route, url.pathname);
      return;
    }

    if (url.protocol !== "data:" && url.protocol !== "blob:") {
      unexpectedRequests.push(`${route.request().method()} ${url.href}`);
      await route.abort("blockedbyclient");
      return;
    }

    await route.continue();
  });

  return unexpectedRequests;
}

async function fulfillRestoreRequest(route: Route) {
  const requestedIds = (await route.request().postDataJSON()) as string[];
  const response = Object.fromEntries(
    videoContents
      .filter(({ contentId }) => requestedIds.includes(contentId))
      .map((video) => [video.contentId, video]),
  );
  await route.fulfill({ json: response });
}

async function fulfillThumbnail(route: Route, pathname: string) {
  const name = path.basename(pathname).split(".")[0];
  const thumbnail = thumbnails[name];
  if (!thumbnail) {
    await route.fulfill({ status: 404 });
    return;
  }

  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="720" viewBox="0 0 1280 720">
    <defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop stop-color="${thumbnail.from}"/><stop offset="1" stop-color="${thumbnail.to}"/></linearGradient></defs>
    <rect width="1280" height="720" fill="url(#g)"/>
    <circle cx="1080" cy="140" r="90" fill="#fff" opacity=".3"/>
    <path d="M0 590 230 390 420 550 650 300 920 570 1280 360V720H0Z" fill="#0f172a" opacity=".35"/>
    <text x="64" y="112" fill="#fff" font-family="sans-serif" font-size="64" font-weight="700">${thumbnail.label}</text>
  </svg>`;
  await route.fulfill({ contentType: "image/svg+xml", body: svg });
}

async function fulfillPlayer(route: Route, pathname: string) {
  const contentId = path.basename(pathname);
  await route.fulfill({
    contentType: "text/html; charset=utf-8",
    body: `<!doctype html><html><body><main><span>MOCK PLAYER</span><strong>${contentId}</strong></main><style>
      html,body{height:100%;margin:0}body{display:grid;place-items:center;background:linear-gradient(135deg,#111827,#312e81);color:white;font-family:sans-serif}
      main{display:grid;gap:1rem;text-align:center}span{font-size:2rem;font-weight:700;letter-spacing:.15em}strong{color:#c4b5fd;font-size:1.25rem}
    </style></body></html>`,
  });
}

async function waitForImages(page: Page) {
  await expect
    .poll(() =>
      page
        .locator(".video_link_component > img")
        .evaluateAll((images) =>
          images.every((image) => (image as HTMLImageElement).complete),
        ),
    )
    .toBe(true);
}

async function capture(page: Page, projectName: string, name: string) {
  const outputDirectory = path.join("vrt-results", projectName);
  await mkdir(outputDirectory, { recursive: true });
  await page.screenshot({
    path: path.join(outputDirectory, `${name}.png`),
    animations: "disabled",
  });
}
