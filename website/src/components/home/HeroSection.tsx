"use client";

import { useEffect, useState } from "react";
import { ArrowRight, Sparkles } from "lucide-react";
import { useTranslations } from "next-intl";

export function HeroSection() {
  const t = useTranslations("home.hero");
  const [visible, setVisible] = useState(false);
  useEffect(() => setVisible(true), []);

  return (
    <section className="hero-gradient grid-pattern relative flex min-h-screen items-center overflow-hidden pt-16">
      {/* Animated orbs */}
      <div className="pointer-events-none absolute inset-0 overflow-hidden">
        <div className="absolute -left-40 top-20 h-80 w-80 animate-pulse-slow rounded-full bg-blue-500/10 blur-[100px]" />
        <div className="absolute -right-40 top-60 h-96 w-96 animate-pulse-slow rounded-full bg-purple-500/10 blur-[100px]" style={{ animationDelay: "2s" }} />
        <div className="absolute bottom-20 left-1/3 h-72 w-72 animate-pulse-slow rounded-full bg-emerald-500/8 blur-[100px]" style={{ animationDelay: "4s" }} />
      </div>

      <div className="container-wide relative z-10 px-4 sm:px-6 lg:px-8">
        <div className="mx-auto max-w-4xl text-center">
          {/* Badge */}
          <div
            className={`mb-8 inline-flex items-center gap-2 rounded-full border border-[var(--glass-border)] bg-[var(--overlay-subtle)] px-4 py-1.5 text-sm t-secondary transition-all duration-700 ${visible ? "translate-y-0 opacity-100" : "translate-y-4 opacity-0"}`}
          >
            <Sparkles size={14} className="text-brand-gold" />
            <span>{t("badge")}</span>
          </div>

          {/* Main heading */}
          <h1
            className={`text-4xl font-bold leading-tight tracking-tight transition-all delay-150 duration-700 sm:text-5xl lg:text-7xl ${visible ? "translate-y-0 opacity-100" : "translate-y-6 opacity-0"}`}
          >
            <span className="gradient-text">{t("heading1")}</span>
            <br />
            <span className="t-primary">{t("heading2")}</span>
          </h1>

          {/* Subtitle */}
          <p
            className={`mx-auto mt-6 max-w-2xl text-lg leading-relaxed t-muted transition-all delay-300 duration-700 sm:text-xl ${visible ? "translate-y-0 opacity-100" : "translate-y-6 opacity-0"}`}
          >
            {t("subtitle")}
          </p>

          {/* CTA buttons */}
          <div
            className={`mt-10 flex flex-col items-center justify-center gap-4 sm:flex-row transition-all delay-500 duration-700 ${visible ? "translate-y-0 opacity-100" : "translate-y-6 opacity-0"}`}
          >
            <a
              href="https://app.nexus.io"
              className="group flex items-center gap-2 rounded-xl bg-gradient-to-r from-blue-600 to-purple-600 px-8 py-3.5 text-sm font-semibold text-white shadow-lg shadow-purple-500/20 transition-all hover:from-blue-500 hover:to-purple-500 hover:shadow-xl hover:shadow-purple-500/30"
            >
              {t("ctaPrimary")}
              <ArrowRight size={16} className="transition-transform group-hover:translate-x-1" />
            </a>
            <a
              href="#three-cores"
              className="flex items-center gap-2 rounded-xl border border-[var(--glass-border)] px-8 py-3.5 text-sm font-semibold t-secondary transition-all hover:border-[var(--glass-hover-border)] hover:bg-[var(--overlay-subtle)] hover:t-primary"
            >
              {t("ctaSecondary")}
            </a>
          </div>

          {/* Key stats */}
          <div
            className={`mt-20 grid grid-cols-2 gap-4 sm:grid-cols-5 transition-all delay-700 duration-700 ${visible ? "translate-y-0 opacity-100" : "translate-y-6 opacity-0"}`}
          >
            {[
              { value: t("stat1Value"), label: t("stat1Label") },
              { value: t("stat2Value"), label: t("stat2Label") },
              { value: t("stat3Value"), label: t("stat3Label") },
              { value: t("stat4Value"), label: t("stat4Label") },
              { value: t("stat5Value"), label: t("stat5Label") },
            ].map((stat) => (
              <div key={stat.label} className="rounded-xl border border-[var(--border-subtle)] bg-[var(--overlay-muted)] px-4 py-4">
                <div className="text-2xl font-bold t-primary">{stat.value}</div>
                <div className="mt-1 text-xs t-faint">{stat.label}</div>
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Bottom fade */}
      <div className="absolute bottom-0 left-0 right-0 h-32 bg-gradient-to-t from-[rgb(var(--bg-primary))] to-transparent" />
    </section>
  );
}
