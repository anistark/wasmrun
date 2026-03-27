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
        'exec/wasi',
        {
          type: 'category',
          label: 'Agent API',
          collapsed: true,
          link: { type: 'doc', id: 'exec/agent' },
          items: [
            'exec/usage/agent-sessions',
            'exec/usage/agent-exec',
            'exec/usage/agent-files',
            'exec/usage/agent-environment',
          ],
        },
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
    {
      type: 'category',
      label: 'Plugins',
      collapsed: false,
      link: { type: 'doc', id: 'plugins/index' },
      items: [
        'plugins/usage',
        {
          type: 'category',
          label: 'Languages',
          collapsed: false,
          items: [
            'plugins/languages/rust',
            'plugins/languages/go',
            'plugins/languages/python',
            'plugins/languages/c-cpp',
            'plugins/languages/assemblyscript',
          ],
        },
        'plugins/creating-plugins',
      ],
    },
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
