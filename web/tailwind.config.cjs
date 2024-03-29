/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./index.html",
    "./src/**/*.{js,ts}",
    "./node_modules/flowbite/**/*.js"
  ],
  darkMode: true,
  theme: {
    extend: {},
  },
  plugins: [
      require('flowbite/plugin')
  ]
}
