import type { SidebarsConfig } from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docs: [
    'intro',
    'installation',
    'quick-start',

    {
      type: 'category',
      label: 'Plugins',
      collapsed: false,
      items: [
        'plugins/index',
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
      ],
    },

    {
      type: 'category',
      label: 'Wasmrun Server',
      collapsed: false,
      items: [
        'server/live-reload',
      ],
    },

    {
      type: 'category',
      label: 'Wasmrun Web',
      collapsed: true,
      items: [
        'web/index',
      ],
    },

    {
      type: 'category',
      label: 'Wasmrun OS',
      collapsed: false,
      items: [
        'os/index',
        'os/network-isolation',
        'os/port-forwarding',
      ],
    },

    {
      type: 'category',
      label: 'Integrations',
      collapsed: false,
      items: [
        'integrations/wasi',
      ],
    },

    {
      type: 'category',
      label: 'CLI Reference',
      collapsed: true,
      items: [
        'cli/index',
        'cli/run',
        'cli/exec',
        'cli/compile',
        'cli/plugin',
        'cli/verify',
        'cli/inspect',
        'cli/clean',
        'cli/stop',
        'cli/os',
      ],
    },

    {
      type: 'category',
      label: 'Development',
      collapsed: true,
      items: [
        'development/architecture',
        'development/creating-plugins',
        'development/contributing',
        'development/debugging',
      ],
    },

    'troubleshooting',
  ],
};

export default sidebars;
