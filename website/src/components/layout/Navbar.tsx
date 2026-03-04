"use client";

import { useState, useEffect } from "react";
import Link from "next/link";
import { Menu, X } from "lucide-react";
import { useTranslations } from "next-intl";
import { cn } from "@/lib/utils";
import { LanguageSwitcher } from "@/components/shared/LanguageSwitcher";
import { ThemeToggle } from "@/components/ui/ThemeToggle";

export function Navbar() {
  const t = useTranslations("nav");
  const [scrolled, setScrolled] = useState(false);
  const [mobileOpen, setMobileOpen] = useState(false);

  const navLinks = [
    { href: "/tokenize", label: t("tokenize") },
    { href: "/growth", label: t("growth") },
    { href: "/ai", label: t("ai") },
    { href: "/stories", label: t("stories") },
    { href: "/tech", label: t("tech") },
    { href: "/docs", label: t("docs") },
  ];

  useEffect(() => {
    const onScroll = () => setScrolled(window.scrollY > 20);
    window.addEventListener("scroll", onScroll);
    return () => window.removeEventListener("scroll", onScroll);
  }, []);

  return (
    <header
      className={cn(
        "fixed top-0 z-50 w-full transition-all duration-300",
        scrolled
          ? "border-b border-[var(--glass-border)] bg-[rgb(var(--bg-primary))]/80 backdrop-blur-xl"
          : "bg-transparent"
      )}
    >
      <nav className="container-wide flex h-16 items-center justify-between px-4 sm:px-6 lg:px-8">
        <Link href="/" className="flex items-center gap-2">
          <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-gradient-to-br from-blue-500 via-purple-500 to-emerald-500">
            <span className="text-sm font-bold text-white">N</span>
          </div>
          <span className="text-lg font-bold tracking-tight">NEXUS</span>
        </Link>

        {/* Desktop nav */}
        <div className="hidden items-center gap-1 md:flex">
          {navLinks.map((link) => (
            <Link
              key={link.href}
              href={link.href}
              className="rounded-lg px-3 py-2 text-sm text-[rgb(var(--text-secondary))] transition-colors hover:bg-[var(--overlay-subtle)] hover:text-[rgb(var(--text-primary))]"
            >
              {link.label}
            </Link>
          ))}
        </div>

        <div className="hidden items-center gap-3 md:flex">
          <ThemeToggle />
          <LanguageSwitcher />
          <Link
            href="/join"
            className="rounded-lg px-4 py-2 text-sm font-medium text-[rgb(var(--text-secondary))] transition-colors hover:text-[rgb(var(--text-primary))]"
          >
            {t("join")}
          </Link>
          <a
            href="https://app.nexus.io"
            className="rounded-lg bg-gradient-to-r from-blue-600 to-purple-600 px-4 py-2 text-sm font-medium text-white transition-all hover:from-blue-500 hover:to-purple-500 hover:shadow-lg hover:shadow-purple-500/25"
          >
            {t("launchDapp")}
          </a>
        </div>

        {/* Mobile toggle */}
        <button
          onClick={() => setMobileOpen(!mobileOpen)}
          className="rounded-lg p-2 text-[rgb(var(--text-secondary))] hover:bg-[var(--overlay-subtle)] md:hidden"
        >
          {mobileOpen ? <X size={20} /> : <Menu size={20} />}
        </button>
      </nav>

      {/* Mobile menu */}
      {mobileOpen && (
        <div className="border-t border-[var(--glass-border)] bg-[rgb(var(--bg-primary))]/95 backdrop-blur-xl md:hidden">
          <div className="space-y-1 px-4 py-4">
            {navLinks.map((link) => (
              <Link
                key={link.href}
                href={link.href}
                onClick={() => setMobileOpen(false)}
                className="block rounded-lg px-3 py-2.5 text-sm text-[rgb(var(--text-secondary))] transition-colors hover:bg-[var(--overlay-subtle)] hover:text-[rgb(var(--text-primary))]"
              >
                {link.label}
              </Link>
            ))}
            <div className="mt-4 flex flex-col gap-2 border-t border-[var(--glass-border)] pt-4">
              <div className="flex items-center justify-center gap-3 py-2">
                <ThemeToggle />
                <LanguageSwitcher />
              </div>
              <Link
                href="/join"
                onClick={() => setMobileOpen(false)}
                className="rounded-lg px-3 py-2.5 text-center text-sm font-medium text-[rgb(var(--text-secondary))]"
              >
                {t("join")}
              </Link>
              <a
                href="https://app.nexus.io"
                className="rounded-lg bg-gradient-to-r from-blue-600 to-purple-600 px-4 py-2.5 text-center text-sm font-medium text-white"
              >
                {t("launchDapp")}
              </a>
            </div>
          </div>
        </div>
      )}
    </header>
  );
}
