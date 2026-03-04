"use client";

import { Shield, Cpu, Database, Globe, Lock, Layers, ArrowRight } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

export default function TechPage() {
  const t = useTranslations("tech");

  const palletGroups = [
    { name: t("g1Name"), count: 15, items: ["registry", "kyc", "token", "shop", "service", "order", "review", "market", "disclosure", "governance", "..."] },
    { name: t("g2Name"), count: 5, items: ["referral-chain", "level-diff", "single-line", "multi-level", "pool-reward"] },
    { name: "GroupRobot", count: 6, items: ["registry", "community", "bot-consensus", "subscription", "ads-campaign", "rewards"] },
    { name: t("g4Name"), count: 3, items: ["escrow", "evidence", "arbitration"] },
    { name: t("g5Name"), count: 4, items: ["p2p-market", "p2p-order", "p2p-credit", "p2p-fiat-verify"] },
    { name: t("g6Name"), count: 7, items: ["membership", "storage", "governance", "treasury", "balances", "staking", "..."] },
  ];

  const techFeatures = [
    { icon: Layers, title: "Substrate L1", desc: t("f1Desc") },
    { icon: Cpu, title: t("f2Title"), desc: t("f2Desc") },
    { icon: Lock, title: "Gramine + SGX", desc: t("f3Desc") },
    { icon: Database, title: t("f4Title"), desc: t("f4Desc") },
    { icon: Shield, title: t("f5Title"), desc: t("f5Desc") },
    { icon: Globe, title: t("f6Title"), desc: t("f6Desc") },
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

      {/* Tech features */}
      <section className="section-padding">
        <div className="container-wide">
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {techFeatures.map((feat) => (
              <div key={feat.title} className="glass-card-hover p-6">
                <div className="mb-4 inline-flex rounded-xl bg-cyan-500/10 p-3">
                  <feat.icon size={22} className="text-cyan-400" />
                </div>
                <h3 className="mb-2 text-lg font-bold">{feat.title}</h3>
                <p className="text-sm leading-relaxed t-muted">{feat.desc}</p>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* Pallet map */}
      <section className="section-padding bg-[var(--overlay-muted)]">
        <div className="container-wide">
          <h2 className="mb-12 text-center text-3xl font-bold">
            <span className="text-cyan-400">40+</span> Runtime Pallets
          </h2>
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {palletGroups.map((group) => (
              <div key={group.name} className="glass-card p-6">
                <div className="mb-3 flex items-center justify-between">
                  <h3 className="font-bold">{group.name}</h3>
                  <span className="rounded-full bg-cyan-500/10 px-2.5 py-0.5 text-xs font-bold text-cyan-400">
                    {group.count}
                  </span>
                </div>
                <div className="flex flex-wrap gap-1.5">
                  {group.items.map((item) => (
                    <span key={item} className="rounded bg-[var(--overlay-subtle)] px-2 py-0.5 text-xs t-muted">
                      {item}
                    </span>
                  ))}
                </div>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* Chain params */}
      <section className="section-padding">
        <div className="container-wide">
          <h2 className="mb-12 text-center text-3xl font-bold">{t("chainTitle")}</h2>
          <div className="mx-auto grid max-w-3xl gap-4 sm:grid-cols-2">
            {[
              { label: t("p1Label"), value: t("p1Value") },
              { label: t("p2Label"), value: t("p2Value") },
              { label: t("p3Label"), value: t("p3Value") },
              { label: "EVM", value: t("p4Value") },
              { label: t("p5Label"), value: t("p5Value") },
              { label: t("p6Label"), value: t("p6Value") },
            ].map((param) => (
              <div key={param.label} className="flex items-center justify-between rounded-xl border border-[var(--border-subtle)] bg-[var(--overlay-muted)] px-6 py-4">
                <span className="text-sm t-muted">{param.label}</span>
                <span className="font-semibold text-cyan-400">{param.value}</span>
              </div>
            ))}
          </div>
        </div>
      </section>

      <section className="section-padding text-center">
        <div className="container-wide">
          <h2 className="text-2xl font-bold">{t("ctaTitle")}</h2>
          <div className="mt-8 flex justify-center gap-4">
            <a href="https://github.com/aspect-build/nexus" className="group flex items-center gap-2 rounded-xl bg-gradient-to-r from-cyan-600 to-cyan-500 px-8 py-3.5 font-semibold text-white">
              GitHub <ArrowRight size={16} className="transition-transform group-hover:translate-x-1" />
            </a>
            <Link href="/join" className="rounded-xl border border-[var(--glass-border)] px-8 py-3.5 font-semibold t-secondary hover:bg-[var(--overlay-subtle)]">
              {t("ctaSecondary")}
            </Link>
          </div>
        </div>
      </section>
    </div>
  );
}
