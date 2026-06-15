import type { Config } from "tailwindcss";

export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        accent: "#F745A1",
        "accent-dim": "rgba(247,69,161,0.5)",
        surface: "#1C1C1E",
        "surface-raised": "#2C2C2E",
        "surface-overlay": "#3A3A3C",
        border: "#3A3A3C",
      },
      fontFamily: {
        sans: [
          "-apple-system",
          "BlinkMacSystemFont",
          "Segoe UI",
          "system-ui",
          "sans-serif",
        ],
      },
    },
  },
  plugins: [],
} satisfies Config;
