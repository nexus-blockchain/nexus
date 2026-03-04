"use client";

import { Moon, Sun } from "lucide-react";
import { useTheme } from "./ThemeProvider";

export function ThemeToggle() {
  const { theme, toggleTheme } = useTheme();

  return (
    <button
      onClick={toggleTheme}
      className="relative flex h-9 w-9 items-center justify-center rounded-lg border border-[var(--glass-border)] bg-[var(--glass-bg)] transition-all hover:bg-[var(--glass-hover-bg)]"
      aria-label={theme === "dark" ? "Switch to light mode" : "Switch to dark mode"}
    >
      <Sun
        size={16}
        className="absolute rotate-0 scale-100 transition-transform duration-300 dark:rotate-90 dark:scale-0"
        style={{
          transform: theme === "dark" ? "rotate(90deg) scale(0)" : "rotate(0) scale(1)",
          opacity: theme === "dark" ? 0 : 1,
        }}
      />
      <Moon
        size={16}
        className="absolute transition-transform duration-300"
        style={{
          transform: theme === "light" ? "rotate(-90deg) scale(0)" : "rotate(0) scale(1)",
          opacity: theme === "light" ? 0 : 1,
        }}
      />
    </button>
  );
}
