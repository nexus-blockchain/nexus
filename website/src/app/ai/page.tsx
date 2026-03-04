"use client";

import { Bot, Brain, LineChart, Cpu, Zap, Shield, ArrowRight } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

export default function AIPage() {
  const t = useTranslations("ai");

  const aiLevels = [
    {
      level: "L1",
      title: t("l1Title"),
      subtitle: t("l1Subtitle"),
      icon: Bot,
      features: [t("l1F1"), t("l1F2"), t("l1F3"), t("l1F4"), t("l1F5")],
      security: [t("l1S1"), t("l1S2"), t("l1S3"), t("l1S4")],
    },
    {
      level: "L2",
      title: t("l2Title"),
      subtitle: t("l2Subtitle"),
      icon: Brain,
      features: [t("l2F1"), t("l2F2"), t("l2F3"), t("l2F4"), t("l2F5")],
    },
    {
      level: "L3",
      title: t("l3Title"),
      subtitle: t("l3Subtitle"),
      icon: LineChart,
      features: [t("l3F1"), t("l3F2"), t("l3F3"), t("l3F4")],
    },
  ];

  const comparison = [
    { dim: t("comp1Dim"), traditional: t("comp1T"), nexus: t("comp1N") },
    { dim: t("comp2Dim"), traditional: t("comp2T"), nexus: t("comp2N") },
    { dim: t("comp3Dim"), traditional: t("comp3T"), nexus: t("comp3N") },
    { dim: t("comp4Dim"), traditional: t("comp4T"), nexus: t("comp4N") },
    { dim: t("comp5Dim"), traditional: t("comp5T"), nexus: t("comp5N") },
  ];

  return (
    <div className="pt-16">
      {/* Hero */}
      <section className="hero-gradient section-padding text-center">
        <div className="container-wide">
          <div className="mb-4 inline-flex items-center gap-2 rounded-full border border-purple-500/20 bg-purple-500/5 px-4 py-1.5 text-sm text-purple-400">
            <Bot size={14} />
            {t("badge")}
          </div>
          <h1 className="mx-auto max-w-3xl text-4xl font-bold leading-tight sm:text-5xl lg:text-6xl">
            <span className="bg-gradient-to-r from-purple-400 to-purple-600 bg-clip-text text-transparent">
              {t("title")}
            </span>
          </h1>
          <p className="mx-auto mt-6 max-w-2xl text-lg t-muted">
            {t("subtitle")}
          </p>
        </div>
      </section>

      {/* Three AI Levels */}
      <section className="section-padding">
        <div className="container-wide space-y-12">
          {aiLevels.map((level) => (
            <div key={level.level} className="glass-card overflow-hidden">
              <div className="border-b border-[var(--border-subtle)] bg-purple-500/5 px-8 py-4">
                <div className="flex items-center gap-3">
                  <span className="rounded-lg bg-purple-500/20 px-2.5 py-1 text-sm font-bold text-purple-400">
                    {level.level}
                  </span>
                  <h3 className="text-xl font-bold">{level.title}</h3>
                </div>
                <p className="mt-1 text-sm t-muted">{level.subtitle}</p>
              </div>
              <div className="p-8">
                <div className={`grid gap-8 ${level.security ? "lg:grid-cols-2" : ""}`}>
                  <div>
                    <h4 className="mb-4 flex items-center gap-2 text-sm font-semibold text-purple-400">
                      <level.icon size={16} />
                      {t("coreCapability")}
                    </h4>
                    <ul className="space-y-2">
                      {level.features.map((f, i) => (
                        <li key={i} className="flex items-start gap-2 text-sm t-secondary">
                          <Zap size={14} className="mt-0.5 shrink-0 text-purple-500/50" />
                          {f}
                        </li>
                      ))}
                    </ul>
                  </div>
                  {level.security && (
                    <div>
                      <h4 className="mb-4 flex items-center gap-2 text-sm font-semibold text-purple-400">
                        <Shield size={16} />
                        {t("security")}
                      </h4>
                      <ul className="space-y-2">
                        {level.security.map((s, i) => (
                          <li key={i} className="flex items-start gap-2 text-sm t-secondary">
                            <Cpu size={14} className="mt-0.5 shrink-0 text-purple-500/50" />
                            {s}
                          </li>
                        ))}
                      </ul>
                    </div>
                  )}
                </div>
              </div>
            </div>
          ))}
        </div>
      </section>

      {/* Comparison */}
      <section className="section-padding bg-[var(--overlay-muted)]">
        <div className="container-wide">
          <h2 className="mb-12 text-center text-3xl font-bold">
            {t("compTitle")} <span className="text-purple-400">{t("compHighlight")}</span>
          </h2>
          <div className="mx-auto max-w-3xl overflow-hidden rounded-xl border border-[var(--glass-border)]">
            <div className="grid grid-cols-3 bg-[var(--overlay-subtle)] px-6 py-3 text-xs font-semibold uppercase tracking-wider t-faint">
              <div>{t("compDim")}</div>
              <div>{t("compTraditional")}</div>
              <div className="text-purple-400">NEXUS (TEE + Chain)</div>
            </div>
            {comparison.map((row, i) => (
              <div key={i} className="grid grid-cols-3 border-t border-[var(--border-subtle)] px-6 py-4">
                <div className="text-sm font-medium t-secondary">{row.dim}</div>
                <div className="text-sm t-faint">{row.traditional}</div>
                <div className="text-sm font-medium text-purple-400">{row.nexus}</div>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* CTA */}
      <section className="section-padding text-center">
        <div className="container-wide">
          <h2 className="text-2xl font-bold">{t("ctaTitle")}</h2>
          <div className="mt-8 flex justify-center gap-4">
            <a href="https://app.nexus.io" className="group flex items-center gap-2 rounded-xl bg-gradient-to-r from-purple-600 to-purple-500 px-8 py-3.5 font-semibold text-white">
              {t("ctaPrimary")} <ArrowRight size={16} className="transition-transform group-hover:translate-x-1" />
            </a>
            <Link href="/stories" className="rounded-xl border border-[var(--glass-border)] px-8 py-3.5 font-semibold t-secondary hover:bg-[var(--overlay-subtle)]">
              {t("ctaSecondary")}
            </Link>
          </div>
        </div>
      </section>
    </div>
  );
}
