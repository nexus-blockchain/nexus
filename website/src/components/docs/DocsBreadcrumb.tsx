"use client";

import Link from "next/link";
import { ChevronRight } from "lucide-react";
import { useTranslations } from "next-intl";

interface DocsBreadcrumbProps {
  category: string;
  section?: string;
  title: string;
}

export function DocsBreadcrumb({ category, section, title }: DocsBreadcrumbProps) {
  const t = useTranslations("docs");

  const categoryLabels: Record<string, string> = {
    business: t("businessCenter"),
    technical: t("technicalDocs"),
  };

  return (
    <nav className="mb-6 flex flex-wrap items-center gap-1 text-sm text-[rgb(var(--text-muted))]">
      <Link href="/docs" className="transition-colors hover:text-[rgb(var(--text-secondary))]">
        {t("title")}
      </Link>
      <ChevronRight size={14} />
      <Link
        href={`/docs/${category}`}
        className="transition-colors hover:text-[rgb(var(--text-secondary))]"
      >
        {categoryLabels[category] || category}
      </Link>
      {section && (
        <>
          <ChevronRight size={14} />
          <span className="capitalize">{section.replace(/-/g, " ")}</span>
        </>
      )}
      <ChevronRight size={14} />
      <span className="text-[rgb(var(--text-primary))] font-medium">{title}</span>
    </nav>
  );
}
