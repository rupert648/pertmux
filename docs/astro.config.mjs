import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  site: 'https://pertmux.dev',
  integrations: [
    starlight({
      title: 'pertmux',
      logo: {
        light: './src/assets/logo-light.svg',
        dark: './src/assets/logo-dark.svg',
        replacesTitle: true,
      },
      social: [
        { icon: 'github', label: 'GitHub', href: 'https://github.com/rupert648/pertmux' },
      ],
      customCss: ['./src/styles/custom.css'],
      sidebar: [
        {
          label: 'Getting Started',
          items: [
            { label: 'Installation', slug: 'getting-started/installation' },
            { label: 'Quick Start', slug: 'getting-started/quick-start' },
            { label: 'tmux Integration', slug: 'getting-started/tmux-integration' },
          ],
        },
        {
          label: 'Configuration',
          items: [
            { label: 'Config Reference', slug: 'configuration/config-reference' },
            { label: 'Multi-Project Setup', slug: 'configuration/multi-project' },
            { label: 'Forge Setup', slug: 'configuration/forge-setup' },
            { label: 'Agent Configuration', slug: 'configuration/agent-config' },
          ],
        },
        {
          label: 'Features',
          items: [
            { label: 'MR Tracking & Linking', slug: 'features/mr-tracking' },
            { label: 'Worktree Management', slug: 'features/worktree-management' },
            { label: 'Agent Monitoring', slug: 'features/agent-monitoring' },
            { label: 'Agent Actions', slug: 'features/agent-actions' },
            { label: 'Pipeline Visualization', slug: 'features/pipeline-visualization' },
          ],
        },
        {
          label: 'Reference',
          items: [
            { label: 'Keybindings', slug: 'reference/keybindings' },
            { label: 'Architecture', slug: 'reference/architecture' },
            { label: 'CLI Commands', slug: 'reference/cli-commands' },
            { label: 'Extending pertmux', slug: 'reference/extending' },
          ],
        },
      ],
    }),
  ],
  vite: {
    plugins: [tailwindcss()],
  },
});
