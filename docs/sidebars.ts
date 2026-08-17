import type { SidebarsConfig } from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docs: [
    'intro',
    'installation',
    'quick-start',
  ],

  server: [
    {
      type: 'category',
      label: 'Server Mode',
      collapsed: false,
      link: { type: 'doc', id: 'server/index' },
      items: [
        'server/features',
        {
          type: 'category',
          label: 'Usage',
          collapsed: false,
          link: { type: 'doc', id: 'server/usage/index' },
          items: [
            'server/usage/run',
            'server/usage/compile',
            'server/usage/verify',
            'server/usage/inspect',
            'server/usage/stop',
            'server/usage/clean',
          ],
        },
        {
          type: 'category',
          label: 'Languages',
          collapsed: false,
          items: [
            'server/languages/rust',
            'server/languages/go',
            'server/languages/python',
            'server/languages/c-cpp',
            'server/languages/assemblyscript',
          ],
        },
        'server/live-reload',
      ],
    },
  ],

  exec: [
    {
      type: 'category',
      label: 'Exec Mode',
      collapsed: false,
      link: { type: 'doc', id: 'exec/index' },
      items: [
        'exec/features',
        {
          type: 'category',
          label: 'Usage',
          collapsed: false,
          link: { type: 'doc', id: 'exec/usage/index' },
          items: [
            'exec/usage/running',
            'exec/usage/functions',
            'exec/usage/arguments',
          ],
        },
        'exec/languages',
        'exec/wasi',
      ],
    },
  ],

  agent: [
    {
      type: 'category',
      label: 'Agent Mode',
      collapsed: false,
      link: { type: 'doc', id: 'agent/index' },
      items: [
        {
          type: 'category',
          label: 'Usage',
          collapsed: false,
          items: [
            'agent/usage/sessions',
            'agent/usage/exec',
            'agent/usage/files',
            'agent/usage/environment',
            'agent/usage/observability',
          ],
        },
        'agent/deployment',
      ],
    },
  ],

  os: [
    {
      type: 'category',
      label: 'OS Mode',
      collapsed: false,
      link: { type: 'doc', id: 'os/index' },
      items: [
        'os/features',
        {
          type: 'category',
          label: 'Usage',
          collapsed: false,
          link: { type: 'doc', id: 'os/usage/index' },
          items: [
            'os/usage/running',
            'os/usage/language',
            'os/usage/server-options',
          ],
        },
        'os/network-isolation',
        'os/port-forwarding',
        'os/public-tunneling',
      ],
    },
  ],

  plugins: [
    'plugins/index',
    'plugins/usage',
    'plugins/c-cpp',
    'plugins/wasmrust',
    'plugins/wasmgo',
    'plugins/wasmasc',
    'plugins/creating-plugins',
  ],

  contributing: [
    {
      type: 'category',
      label: 'Contributing',
      collapsed: false,
      link: { type: 'doc', id: 'contributing/index' },
      items: [
        'contributing/architecture',
        'contributing/how-to-contribute',
        'contributing/debugging',
        'contributing/troubleshooting',
        'contributing/changelog',
      ],
    },
  ],
};

export default sidebars;
