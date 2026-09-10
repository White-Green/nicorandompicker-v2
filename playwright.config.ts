import { defineConfig } from "@playwright/test";

const colorSchemes = ["light", "dark"] as const;

const viewportProjects = [
  {
    name: "desktop-landscape",
    use: { viewport: { width: 1440, height: 900 } },
  },
  {
    name: "desktop-portrait",
    use: { viewport: { width: 900, height: 1440 } },
  },
  {
    name: "mobile-landscape",
    use: {
      viewport: { width: 844, height: 390 },
      deviceScaleFactor: 1,
      hasTouch: true,
      isMobile: true,
    },
  },
  {
    name: "mobile-portrait",
    use: {
      viewport: { width: 390, height: 844 },
      deviceScaleFactor: 1,
      hasTouch: true,
      isMobile: true,
    },
  },
];

export default defineConfig({
  testDir: "./tests/vrt",
  fullyParallel: false,
  workers: 1,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 1 : 0,
  reporter: "line",
  use: {
    baseURL: "http://127.0.0.1:4173",
    locale: "ja-JP",
    timezoneId: "Asia/Tokyo",
  },
  projects: viewportProjects.flatMap((project) =>
    colorSchemes.map((colorScheme) => ({
      ...project,
      name: `${project.name}-${colorScheme}`,
      use: { ...project.use, colorScheme },
    })),
  ),
  webServer: {
    command: "node node_modules/vite/bin/vite.js --host 127.0.0.1 --port 4173",
    url: "http://127.0.0.1:4173",
    reuseExistingServer: !process.env.CI,
  },
});
