import starlightPlugin from '@astrojs/starlight-tailwind';

const accent = {
  200: '#ffbd6b',
  600: '#cc6e00',
  900: '#5c3200',
  950: '#422300',
};

const gray = {
  100: '#f5f6f8',
  200: '#eceef2',
  300: '#c0c2c7',
  400: '#888b96',
  500: '#545861',
  700: '#353841',
  800: '#24272f',
  900: '#17191e',
  950: '#0e1015',
};

/** @type {import('tailwindcss').Config} */
export default {
  content: ['./src/**/*.{astro,html,js,jsx,md,mdx,svelte,ts,tsx,vue}'],
  theme: {
    extend: {
      colors: {
        accent,
        gray,
        orange: {
          400: '#ff8c00',
          500: '#e07d00',
          600: '#cc6e00',
        },
      },
      fontFamily: {
        sans: ['Outfit', 'system-ui', 'sans-serif'],
        mono: ['JetBrains Mono', 'Fira Code', 'monospace'],
      },
    },
  },
  plugins: [starlightPlugin()],
};
