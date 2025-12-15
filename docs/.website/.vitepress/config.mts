import { createRequire } from "module";
import { defineConfig } from "vitepress";

const require = createRequire(import.meta.url);

export default defineConfig({
  lang: "en-US",
  title: "Serene",
  description: "Documentation of serene package build server",
  srcDir: "../",
  srcExclude: [".website/**"],
  rewrites: {
    // only rewrite main readme file since there are no backlinks to it
    // the problem with backlinks is that they need to contain the rewritten
    // link as can be read here: https://vitepress.dev/guide/routing#route-rewrites
    "readme.md": "index.md",
  },
  base: "/serene-aur",
  cleanUrls: true,
  themeConfig: {
    editLink: {
      pattern: "https://github.com/VirtCode/serene-aur/edit/main/docs/:path",
      text: "Edit this page on GitHub",
    },
    nav: [
      { text: "Documentation", link: "/", activeMatch: "/.*" },
      {
        text: "File an issue",
        link: "https://github.com/VirtCode/serene-aur/issues",
      },
    ],

    sidebar: [
      { text: "Introduction", link: "/" },
      {
        text: "Usage",
        base: "/usage/",
        items: [
          { text: "Overview", link: "readme" },
          { text: "Command Line Interface", link: "cli" },
          { text: "Package Sources", link: "package-sources" },
        ],
      },
      {
        text: "Configuration",
        base: "/configuration/",
        items: [
          { text: "Overview", link: "readme" },
          { text: "Dependency Resolving", link: "dependency-resolving" },
          { text: "Package Signing", link: "package-signing" },
          { text: "Webhooks", link: "webhooks" },
          { text: "GitHub Mirror", link: "github-mirror" },
        ],
      },
      {
        text: "Deployment",
        base: "/deployment/",
        items: [
          { text: "Overview", link: "readme" },
          { text: "Using Host Docker", link: "host-docker" },
          { text: "Using Docker In Docker", link: "docker-in-docker" },
        ],
      },
    ],
    socialLinks: [
      { icon: "github", link: "https://github.com/virtcode/serene-aur/" },
    ],
    search: {
      provider: "local",
    },
  },
  vite: {
    // this is needed due to this issue:
    // https://github.com/vuejs/vitepress/issues/4612
    plugins: [
      {
        name: "node-resolve-from-different-root",
        resolveId(id) {
          try {
            const resolve = require.resolve(id);
            if (resolve) return { id: resolve };
          } catch (e) {}
        },
      },
    ],
  },
});
