import type { SidebarsConfig } from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docs: [
    'intro',
    'installation',
    'quick-start',

    {
      type: 'category',
      label: 'Language Guides',
      collapsed: false,
      items: [
        'guides/rust',
        'guides/go',
        'guides/python',
        'guides/c-cpp',
        'guides/assemblyscript',
      ],
    },

    {
      type: 'category',
      label: 'Features',
      collapsed: false,
      items: [
        'features/plugin-system',
        'features/live-reload',
        'features/native-execution',
        'features/wasi-support',
        'features/network-isolation',
        'features/port-forwarding',
        'features/os-mode',
      ],
    },

    {
      type: 'category',
      label: 'CLI Reference',
      collapsed: true,
      items: [
        'cli/overview',
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
    'changelog',
  ],
};

export default sidebars;
