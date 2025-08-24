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
    ":section/readme.md": ":section/index.md",
    "readme.md": "index.md",
  },
  base: "/serene-aur/docs",
  cleanUrls: true,
  // we need to ignore dead links since there are some links referring to code files
  ignoreDeadLinks: true,
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
        items: [
          { text: "Overview", link: "/usage/" },
          { text: "Command Line Interface", link: "/usage/cli" },
          { text: "Package Sources", link: "/usage/package-sources" },
        ],
      },
      {
        text: "Deployment",
        items: [
          { text: "Overview", link: "/deployment/" },
          { text: "Using Host Docker", link: "/deployment/host-docker" },
          {
            text: "Using Docker In Docker",
            link: "/deployment/docker-in-docker",
          },
        ],
      },
      {
        text: "Configuration",
        items: [
          { text: "Overview", link: "/configuration/" },
          {
            text: "Dependency Resolving",
            link: "/configuration/dependency-resolving",
          },
          { text: "Package Signing", link: "/configuration/package-signing" },
          { text: "Webhooks", link: "/configuration/webhooks" },
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
