/** @type {import('tailwindcss').Config} */
export default {
  content: ['./src/**/*.{vue,js,ts}'],
  // Keep Tailwind utilities inside the package boundary. Host applications may
  // use the same utility class names with a different Tailwind version/config.
  important: '.fpp-ui',
  theme: {
    extend: {
      animation: { 'fade-in': 'fpp-fade-in .4s ease-out forwards' },
      keyframes: { 'fpp-fade-in': { from: { opacity: '0', transform: 'translateY(10px)' }, to: { opacity: '1', transform: 'translateY(0)' } } },
    },
  },
  plugins: [],
}
