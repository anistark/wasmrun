import type { Config } from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';
import { themes as prismThemes } from 'prism-react-renderer';

const config: Config = {
  title: 'Wasmrun',
  tagline: 'WebAssembly Development Server',
  url: 'https://wasmrun.readthedocs.io',
  // Use /en/latest/ for production (ReadTheDocs), / for local development
  baseUrl: process.env.NODE_ENV === 'production' ? '/en/latest/' : '/',

  favicon: 'img/favicon.ico',
  organizationName: 'anistark',
  projectName: 'wasmrun',

  onBrokenLinks: 'throw',
  onBrokenMarkdownLinks: 'warn',

  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  presets: [
    [
      'classic',
      {
        docs: {
          sidebarPath: './sidebars.ts',
          editUrl: 'https://github.com/anistark/wasmrun/edit/main/docs/',
          showLastUpdateAuthor: true,
          showLastUpdateTime: true,
          versions: {
            current: {
              label: 'Next 🚧',
            },
          },
        },
        blog: false,
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    image: 'img/banner.png',
    navbar: {
      title: 'Wasmrun',
      logo: {
        alt: 'Wasmrun Logo',
        src: 'img/logo.png',
      },
      items: [
        {
          type: 'docSidebar',
          sidebarId: 'docs',
          position: 'left',
          label: 'Docs',
        },
        {
          to: '/tutorials',
          label: 'Tutorials',
          position: 'left',
        },
        {
          to: '/community',
          label: 'Community',
          position: 'left',
        },
        {
          type: 'docsVersionDropdown',
          position: 'right',
        },
        {
          href: 'https://docs.rs/wasmrun',
          label: 'API',
          position: 'right',
        },
        {
          href: 'https://github.com/anistark/wasmrun',
          label: 'GitHub',
          position: 'right',
        },
      ],
    },

    footer: {
      style: 'dark',
      links: [
        {
          title: 'Quick Links',
          items: [
            { label: 'Introduction', to: '/docs/intro' },
            { label: 'Installation', to: '/docs/installation' },
            { label: 'Quick Start', to: '/docs/quick-start' },
            { label: 'Rust Guide', to: '/docs/guides/rust' },
            { label: 'Go Guide', to: '/docs/guides/go' },
            { label: 'Python Guide', to: '/docs/guides/python' },
            { label: 'C/C++ Guide', to: '/docs/guides/c-cpp' },
          ],
        },
        {
          title: 'Community',
          items: [
            { label: 'Community', to: '/community' },
            { label: 'Issues', href: 'https://github.com/anistark/wasmrun/issues' },
            { label: 'Discussions', href: 'https://github.com/anistark/wasmrun/discussions' },
          ],
        },
        {
          title: 'Resources',
          items: [
            { label: 'Tutorials', to: '/tutorials' },
            { label: 'Rust API Docs', href: 'https://docs.rs/wasmrun' },
            { label: 'Crates.io', href: 'https://crates.io/crates/wasmrun' },
          ],
        },
        {
          title: 'Social',
          items: [
            {
              label: 'Twitter',
              href: 'https://twitter.com/wasmrun',
            },
            {
              label: 'GitHub',
              href: 'https://github.com/anistark/wasmrun',
            },
            {
              label: 'Mastodon',
              href: 'https://mastodon.social/@wasmrun',
            },
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} Wasmrun Team.`,
    },

    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
      additionalLanguages: ['rust', 'go', 'python', 'bash', 'toml', 'json'],
    },
  } satisfies Preset.ThemeConfig,

  plugins: [
    [
      '@docusaurus/plugin-content-pages',
      {
        id: 'tutorials',
        path: 'tutorials',
        routeBasePath: 'tutorials',
      },
    ],
  ],
};

export default config;
