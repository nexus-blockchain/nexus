"use client";

import {
  Megaphone,
  Users,
  Target,
  TrendingUp,
  ArrowRight,
  CheckCircle2,
  GitBranch,
} from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

export default function GrowthPage() {
  const t = useTranslations("growth");

  const commissionModes = [
    { name: t("m1Name"), tag: "Referral", desc: t("m1Desc"), scenario: t("m1Scenario") },
    { name: t("m2Name"), tag: "Level-Diff", desc: t("m2Desc"), scenario: t("m2Scenario") },
    { name: t("m3Name"), tag: "Single-Line", desc: t("m3Desc"), scenario: t("m3Scenario") },
    { name: t("m4Name"), tag: "Multi-Level", desc: t("m4Desc"), scenario: t("m4Scenario") },
    { name: t("m5Name"), tag: "Pool Reward", desc: t("m5Desc"), scenario: t("m5Scenario") },
  ];

  const upgradeTriggers = [
    { trigger: t("t1"), field: "PurchaseProduct / TotalSpent" },
    { trigger: t("t2"), field: "ReferralCount" },
    { trigger: t("t3"), field: "TeamSize" },
    { trigger: t("t4"), field: "SingleOrder / SingleOrderUsdt" },
    { trigger: t("t5"), field: "TotalSpentUsdt" },
    { trigger: t("t6"), field: "ReferralLevelCount" },
  ];

  return (
    <div className="pt-16">
      {/* Hero */}
      <section className="hero-gradient section-padding text-center">
        <div className="container-wide">
          <div className="mb-4 inline-flex items-center gap-2 rounded-full border border-emerald-500/20 bg-emerald-500/5 px-4 py-1.5 text-sm text-emerald-400">
            <Megaphone size={14} />
            {t("badge")}
          </div>
          <h1 className="mx-auto max-w-3xl text-4xl font-bold leading-tight sm:text-5xl lg:text-6xl">
            <span className="bg-gradient-to-r from-emerald-400 to-emerald-600 bg-clip-text text-transparent">
              {t("title")}
            </span>
          </h1>
          <p className="mx-auto mt-6 max-w-2xl text-lg t-muted">
            {t("subtitle")}
          </p>
        </div>
      </section>

      {/* 5 Commission Modes */}
      <section className="section-padding">
        <div className="container-wide">
          <h2 className="mb-4 text-center text-3xl font-bold">
            <span className="text-emerald-400">{t("modesCount")}</span> {t("modesTitle")}
          </h2>
          <p className="mb-12 text-center t-muted">
            {t("modesSubtitle")}
          </p>
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-5">
            {commissionModes.map((mode) => (
              <div key={mode.name} className="glass-card-hover border-emerald-500/10 p-6 hover:border-emerald-500/25">
                <span className="mb-3 inline-block rounded-md bg-emerald-500/10 px-2 py-0.5 text-xs text-emerald-400">
                  {mode.tag}
                </span>
                <h3 className="mb-2 text-lg font-bold">{mode.name}</h3>
                <p className="mb-3 text-sm leading-relaxed t-muted">{mode.desc}</p>
                <p className="text-xs text-emerald-400/60">{mode.scenario}</p>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* Member Upgrade */}
      <section className="section-padding bg-[var(--overlay-muted)]">
        <div className="container-wide">
          <h2 className="mb-4 text-center text-3xl font-bold">
            {t("flywheel")} <span className="text-emerald-400">{t("flywheelHighlight")}</span>
          </h2>
          <p className="mb-12 text-center t-muted">
            {t("flywheelSubtitle")}
          </p>

          <div className="grid gap-8 lg:grid-cols-2">
            {/* Triggers */}
            <div className="glass-card p-8">
              <h3 className="mb-6 flex items-center gap-2 text-lg font-bold">
                <Users size={20} className="text-emerald-400" />
                {t("triggersTitle")}
              </h3>
              <div className="space-y-3">
                {upgradeTriggers.map((item) => (
                  <div key={item.trigger} className="flex items-center justify-between rounded-lg border border-[var(--border-subtle)] bg-[var(--overlay-muted)] px-4 py-3">
                    <span className="text-sm t-secondary">{item.trigger}</span>
                    <span className="rounded bg-emerald-500/10 px-2 py-0.5 text-xs text-emerald-400">{item.field}</span>
                  </div>
                ))}
              </div>
            </div>

            {/* Spillover */}
            <div className="glass-card p-8">
              <h3 className="mb-6 flex items-center gap-2 text-lg font-bold">
                <GitBranch size={20} className="text-emerald-400" />
                {t("spilloverTitle")}
              </h3>
              <div className="space-y-4 text-sm t-secondary">
                <p>{t("spilloverDesc")}</p>
                <div className="rounded-lg border border-emerald-500/10 bg-emerald-500/5 p-4">
                  <p className="mb-2 font-semibold text-emerald-400">{t("spilloverHow")}</p>
                  <ul className="space-y-1.5">
                    <li className="flex items-start gap-2">
                      <CheckCircle2 size={14} className="mt-0.5 shrink-0 text-emerald-500" />
                      {t("spilloverS1")}
                    </li>
                    <li className="flex items-start gap-2">
                      <CheckCircle2 size={14} className="mt-0.5 shrink-0 text-emerald-500" />
                      {t("spilloverS2")}
                    </li>
                    <li className="flex items-start gap-2">
                      <CheckCircle2 size={14} className="mt-0.5 shrink-0 text-emerald-500" />
                      {t("spilloverS3")}
                    </li>
                  </ul>
                </div>
                <p className="text-emerald-400">{t("spilloverResult")}</p>
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* Ads + P2P */}
      <section className="section-padding">
        <div className="container-wide">
          <div className="grid gap-6 md:grid-cols-2">
            <div className="glass-card p-8">
              <div className="mb-4 inline-flex rounded-xl bg-emerald-500/10 p-3">
                <Target size={24} className="text-emerald-400" />
              </div>
              <h3 className="mb-2 text-xl font-bold">{t("adsTitle")}</h3>
              <ul className="space-y-2 text-sm t-secondary">
                <li>{t("adsL1")}</li>
                <li>{t("adsL2")}</li>
                <li>{t("adsL3")}</li>
                <li>{t("adsL4")}</li>
              </ul>
            </div>
            <div className="glass-card p-8">
              <div className="mb-4 inline-flex rounded-xl bg-emerald-500/10 p-3">
                <TrendingUp size={24} className="text-emerald-400" />
              </div>
              <h3 className="mb-2 text-xl font-bold">{t("p2pTitle")}</h3>
              <ul className="space-y-2 text-sm text-white/60">
                <li>{t("p2pL1")}</li>
                <li>{t("p2pL2")}</li>
                <li>{t("p2pL3")}</li>
                <li>{t("p2pL4")}</li>
              </ul>
            </div>
          </div>
        </div>
      </section>

      {/* CTA */}
      <section className="section-padding text-center">
        <div className="container-wide">
          <h2 className="text-2xl font-bold">{t("ctaTitle")}</h2>
          <div className="mt-8 flex justify-center gap-4">
            <a href="https://app.nexus.io" className="group flex items-center gap-2 rounded-xl bg-gradient-to-r from-emerald-600 to-emerald-500 px-8 py-3.5 font-semibold text-white">
              {t("ctaPrimary")} <ArrowRight size={16} className="transition-transform group-hover:translate-x-1" />
            </a>
            <Link href="/ai" className="rounded-xl border border-[var(--glass-border)] px-8 py-3.5 font-semibold t-secondary hover:bg-[var(--overlay-subtle)]">
              {t("ctaSecondary")}
            </Link>
          </div>
        </div>
      </section>
    </div>
  );
}
