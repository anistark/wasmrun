import type { Config } from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';
import type { PrismTheme } from 'prism-react-renderer';

// Custom Prism themes in the site's palette: neutral surfaces with an
// emerald anchor, instead of the stock github/dracula pairing.
const wasmrunPrismLight: PrismTheme = {
  plain: { color: '#2b2925', backgroundColor: '#f4f0e7' },
  styles: [
    { types: ['comment', 'prolog', 'cdata'], style: { color: '#6e7781', fontStyle: 'italic' } },
    { types: ['keyword', 'atrule', 'selector', 'important'], style: { color: '#047857' } },
    { types: ['string', 'char', 'inserted', 'attr-value', 'url'], style: { color: '#0a7ea4' } },
    { types: ['function', 'function-variable'], style: { color: '#6f42c1' } },
    { types: ['number', 'boolean', 'constant', 'symbol', 'deleted'], style: { color: '#b35900' } },
    { types: ['punctuation', 'operator'], style: { color: '#57606a' } },
    { types: ['class-name', 'builtin', 'namespace', 'tag'], style: { color: '#116329' } },
    { types: ['variable', 'property', 'attr-name'], style: { color: '#24292f' } },
  ],
};

const wasmrunPrismDark: PrismTheme = {
  plain: { color: '#d4dcd8', backgroundColor: '#0d1512' },
  styles: [
    { types: ['comment', 'prolog', 'cdata'], style: { color: '#5f6f68', fontStyle: 'italic' } },
    { types: ['keyword', 'atrule', 'selector', 'important'], style: { color: '#34d399' } },
    { types: ['string', 'char', 'inserted', 'attr-value', 'url'], style: { color: '#7dd3fc' } },
    { types: ['function', 'function-variable'], style: { color: '#c4b5fd' } },
    { types: ['number', 'boolean', 'constant', 'symbol', 'deleted'], style: { color: '#fbbf24' } },
    { types: ['punctuation', 'operator'], style: { color: '#8b9a93' } },
    { types: ['class-name', 'builtin', 'namespace', 'tag'], style: { color: '#6ee7b7' } },
    { types: ['variable', 'property', 'attr-name'], style: { color: '#d4dcd8' } },
  ],
};

// ReadTheDocs serves the site under /<language>/<version>/ (e.g. /en/latest/)
// and sets these env vars only inside its build environment. Everywhere else
// — the local dev server and local production builds (`pnpm build && serve`) —
// the site is served from the root, so baseUrl must stay '/'. Keying off
// NODE_ENV is wrong because any production build sets it, including local ones.
const isReadTheDocs = process.env.READTHEDOCS === 'True';
const rtdLanguage = process.env.READTHEDOCS_LANGUAGE || 'en';
const rtdVersion = process.env.READTHEDOCS_VERSION || 'latest';

const config: Config = {
  title: 'Wasmrun',
  tagline: 'WebAssembly Runtime',
  url: 'https://wasmrun.readthedocs.io',
  baseUrl: isReadTheDocs ? `/${rtdLanguage}/${rtdVersion}/` : '/',

  favicon: 'img/favicon.ico',
  organizationName: 'anistark',
  projectName: 'wasmrun',

  onBrokenLinks: 'throw',

  stylesheets: [
    {
      href: 'https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=Space+Grotesk:wght@500;600;700&family=JetBrains+Mono:wght@400;500;600&display=swap',
      type: 'text/css',
    },
  ],

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
    colorMode: {
      defaultMode: 'dark',
      respectPrefersColorScheme: false,
    },
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
          position: 'right',
          className: 'header-github-link',
          'aria-label': 'GitHub repository',
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
      theme: wasmrunPrismLight,
      darkTheme: wasmrunPrismDark,
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
