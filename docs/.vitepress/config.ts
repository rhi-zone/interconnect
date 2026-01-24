import { defineConfig } from 'vitepress'
import { withMermaid } from 'vitepress-plugin-mermaid'

export default withMermaid(
  defineConfig({
    title: 'Interconnect',
    description: 'Federation protocol for persistent worlds',

    base: '/interconnect/',

    themeConfig: {
      nav: [
        { text: 'Guide', link: '/introduction' },
        { text: 'Protocol', link: '/protocol' },
        { text: 'Security', link: '/security' },
        { text: 'Rhi', link: 'https://rhi.zone/' },
      ],

      sidebar: [
        {
          text: 'Guide',
          items: [
            { text: 'Introduction', link: '/introduction' },
            { text: 'Architecture', link: '/architecture' },
          ]
        },
        {
          text: 'Reference',
          items: [
            { text: 'Protocol', link: '/protocol' },
            { text: 'Security', link: '/security' },
            { text: 'Import Policies', link: '/import-policies' },
          ]
        },
      ],

      socialLinks: [
        { icon: 'github', link: 'https://github.com/rhi-zone/interconnect' }
      ],

      search: {
        provider: 'local'
      },

      editLink: {
        pattern: 'https://github.com/rhi-zone/interconnect/edit/master/docs/:path',
        text: 'Edit this page on GitHub'
      },
    },

    vite: {
      optimizeDeps: {
        include: ['mermaid'],
      },
    },
  }),
)
