import type { Config } from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';
import { themes as prismThemes } from 'prism-react-renderer';

const config: Config = {
  title: 'Wasmrun',
  tagline: 'WebAssembly Runtime',
  url: 'https://wasmrun.readthedocs.io',
  baseUrl: process.env.NODE_ENV === 'production'
    ? `/en/${process.env.READTHEDOCS_VERSION || 'latest'}/`
    : '/',

  favicon: 'img/favicon.ico',
  organizationName: 'anistark',
  projectName: 'wasmrun',

  onBrokenLinks: 'throw',

  markdown: {
    hooks: {
      onBrokenMarkdownLinks: 'warn',
    },
  },

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
          sidebarId: 'server',
          position: 'left',
          label: 'Server',
        },
        {
          type: 'docSidebar',
          sidebarId: 'exec',
          position: 'left',
          label: 'Exec',
        },
        {
          type: 'docSidebar',
          sidebarId: 'os',
          position: 'left',
          label: 'OS',
        },
        {
          type: 'docSidebar',
          sidebarId: 'plugins',
          position: 'left',
          label: 'Plugins',
        },
        {
          type: 'docSidebar',
          sidebarId: 'contributing',
          position: 'left',
          label: 'Contributing',
        },
        {
          type: 'dropdown',
          label: 'Related',
          position: 'left',
          items: [
            {
              type: 'docSidebar',
              sidebarId: 'wasmhub',
              label: 'WasmHub',
            },
          ],
        },
        // {
        //   to: '/tutorials',
        //   label: 'Tutorials',
        //   position: 'left',
        // },
        {
          to: '/community',
          label: 'Community',
          position: 'left',
        },
        {
          href: 'https://docs.rs/wasmrun',
          label: 'API',
          position: 'right',
        },
        {
          href: 'https://crates.io/crates/wasmrun',
          label: 'Crate',
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
          title: 'Docs',
          items: [
            { label: 'Server Mode', to: '/docs/server' },
            { label: 'Exec Mode', to: '/docs/exec' },
            { label: 'OS Mode', to: '/docs/os' },
            { label: 'Plugins', to: '/docs/plugins' },
            { label: 'WasmHub', to: '/docs/wasmhub' },
            { label: 'Changelog', to: '/docs/contributing/changelog' },
          ],
        },
        {
          title: 'Community',
          items: [
            { label: 'Overview', to: '/community' },
            { label: 'Issues', href: 'https://github.com/anistark/wasmrun/issues' },
            { label: 'Discussions', href: 'https://github.com/anistark/wasmrun/discussions' },
          ],
        },
        {
          title: 'Resources',
          items: [
            { label: 'Installation', to: '/docs/installation' },
            { label: 'docs.rs', href: 'https://docs.rs/wasmrun' },
            { label: 'lib.rs', href: 'https://lib.rs/wasmrun' },
            { label: 'crates.io', href: 'https://crates.io/crates/wasmrun' },
          ],
        },
        {
          title: 'Social',
          items: [
            {
              label: 'X (Twitter)',
              href: 'https://x.com/kranirudha',
            },
            {
              label: 'GitHub',
              href: 'https://github.com/anistark/wasmrun',
            },
            {
              label: 'Fediverse',
              href: 'https://fosstodon.org/@ani',
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
