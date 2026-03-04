"use client";

import { ArrowRight } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

const colorMap: Record<string, { border: string; badge: string; metric: string }> = {
  blue: { border: "border-blue-500/20", badge: "bg-blue-500/10 text-blue-400", metric: "text-blue-400" },
  green: { border: "border-emerald-500/20", badge: "bg-emerald-500/10 text-emerald-400", metric: "text-emerald-400" },
  purple: { border: "border-purple-500/20", badge: "bg-purple-500/10 text-purple-400", metric: "text-purple-400" },
};

export default function StoriesPage() {
  const t = useTranslations("stories");

  const stories = [
    {
      title: t("s1Title"),
      subtitle: t("s1Subtitle"),
      color: "blue",
      before: [t("s1B1"), t("s1B2"), t("s1B3")],
      after: [t("s1A1"), t("s1A2"), t("s1A3"), t("s1A4")],
      metrics: [
        { label: t("s1M1Label"), value: t("s1M1Value") },
        { label: t("s1M2Label"), value: t("s1M2Value") },
        { label: t("s1M3Label"), value: t("s1M3Value") },
      ],
    },
    {
      title: t("s2Title"),
      subtitle: t("s2Subtitle"),
      color: "green",
      before: [t("s2B1"), t("s2B2"), t("s2B3")],
      after: [t("s2A1"), t("s2A2"), t("s2A3"), t("s2A4")],
      metrics: [
        { label: t("s2M1Label"), value: t("s2M1Value") },
        { label: t("s2M2Label"), value: t("s2M2Value") },
        { label: t("s2M3Label"), value: t("s2M3Value") },
      ],
    },
    {
      title: t("s3Title"),
      subtitle: t("s3Subtitle"),
      color: "purple",
      before: [t("s3B1"), t("s3B2"), t("s3B3")],
      after: [t("s3A1"), t("s3A2"), t("s3A3"), t("s3A4")],
      metrics: [
        { label: t("s3M1Label"), value: t("s3M1Value") },
        { label: t("s3M2Label"), value: t("s3M2Value") },
        { label: t("s3M3Label"), value: t("s3M3Value") },
      ],
    },
  ];

  return (
    <div className="pt-16">
      <section className="hero-gradient section-padding text-center">
        <div className="container-wide">
          <h1 className="text-4xl font-bold sm:text-5xl">
            {t("title")} <span className="gradient-text">{t("titleHighlight")}</span>
          </h1>
          <p className="mx-auto mt-6 max-w-2xl text-lg t-muted">
            {t("subtitle")}
          </p>
        </div>
      </section>

      <section className="section-padding">
        <div className="container-wide space-y-12">
          {stories.map((story) => {
            const colors = colorMap[story.color];
            return (
              <div key={story.title} className={`glass-card overflow-hidden ${colors.border}`}>
                <div className="border-b border-[var(--border-subtle)] px-8 py-6">
                  <span className={`mb-2 inline-block rounded-md px-2 py-0.5 text-xs font-medium ${colors.badge}`}>
                    {t("caseBadge")}
                  </span>
                  <h2 className="text-2xl font-bold">{story.title}</h2>
                  <p className="mt-1 t-muted">{story.subtitle}</p>
                </div>

                <div className="grid gap-8 p-8 lg:grid-cols-3">
                  <div>
                    <h3 className="mb-4 text-sm font-semibold t-faint">{t("beforeLabel")}</h3>
                    <ul className="space-y-2">
                      {story.before.map((b, i) => (
                        <li key={i} className="flex items-start gap-2 text-sm t-muted">
                          <span className="mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full bg-red-500/50" />
                          {b}
                        </li>
                      ))}
                    </ul>
                  </div>

                  <div>
                    <h3 className={`mb-4 text-sm font-semibold ${colors.metric}`}>{t("afterLabel")}</h3>
                    <ul className="space-y-2">
                      {story.after.map((a, i) => (
                        <li key={i} className="flex items-start gap-2 text-sm t-secondary">
                          <span className={`mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full ${colors.badge.split(" ")[0]}`} />
                          {a}
                        </li>
                      ))}
                    </ul>
                  </div>

                  <div>
                    <h3 className="mb-4 text-sm font-semibold t-faint">{t("metricsLabel")}</h3>
                    <div className="space-y-3">
                      {story.metrics.map((m) => (
                        <div key={m.label} className="rounded-lg border border-[var(--border-subtle)] bg-[var(--overlay-muted)] px-4 py-3">
                          <div className={`text-2xl font-bold ${colors.metric}`}>{m.value}</div>
                          <div className="text-xs t-faint">{m.label}</div>
                        </div>
                      ))}
                    </div>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      </section>

      <section className="section-padding text-center">
        <div className="container-wide">
          <h2 className="text-2xl font-bold">{t("ctaTitle")}</h2>
          <div className="mt-8 flex justify-center gap-4">
            <a href="https://app.nexus.io" className="group flex items-center gap-2 rounded-xl bg-gradient-to-r from-blue-600 to-purple-600 px-8 py-3.5 font-semibold text-white">
              {t("ctaPrimary")} <ArrowRight size={16} className="transition-transform group-hover:translate-x-1" />
            </a>
            <Link href="/tech" className="rounded-xl border border-[var(--glass-border)] px-8 py-3.5 font-semibold t-secondary hover:bg-[var(--overlay-subtle)]">
              {t("ctaSecondary")}
            </Link>
          </div>
        </div>
      </section>
    </div>
  );
}
