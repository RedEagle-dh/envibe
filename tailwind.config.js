/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        'envibe': {
          bg: '#0d1117',
          'bg-secondary': '#161b22',
          'bg-tertiary': '#21262d',
          border: '#30363d',
          'border-muted': '#21262d',
          text: '#c9d1d9',
          'text-muted': '#8b949e',
          'text-subtle': '#6e7681',
          accent: '#58a6ff',
          'accent-emphasis': '#388bfd',
          success: '#3fb950',
          'success-emphasis': '#238636',
          warning: '#d29922',
          'warning-emphasis': '#9e6a03',
          danger: '#f85149',
          'danger-emphasis': '#da3633',
        },
      },
      fontFamily: {
        mono: ['JetBrains Mono', 'Fira Code', 'Monaco', 'Consolas', 'monospace'],
      },
    },
  },
  plugins: [],
};
