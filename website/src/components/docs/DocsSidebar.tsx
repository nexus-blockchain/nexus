"use client";

import { useState } from "react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { useTranslations } from "next-intl";
import {
  ChevronDown,
  ChevronRight,
  TrendingUp,
  Wrench,
  FileText,
  Building,
  Coins,
  Users,
  Bot,
  Shield,
  Store,
  Rocket,
  HelpCircle,
  BookOpen,
  Layers,
  Database,
  Globe,
  Code,
  Server,
  Lock,
  GitBranch,
  X,
} from "lucide-react";
import { cn } from "@/lib/utils";
import type { SidebarSection, SidebarItem } from "@/lib/docs";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const iconMap: Record<string, React.ComponentType<any>> = {
  "trending-up": TrendingUp,
  wrench: Wrench,
  "file-text": FileText,
  building: Building,
  coins: Coins,
  users: Users,
  bot: Bot,
  shield: Shield,
  store: Store,
  rocket: Rocket,
  "help-circle": HelpCircle,
  "book-open": BookOpen,
  layers: Layers,
  database: Database,
  globe: Globe,
  code: Code,
  server: Server,
  lock: Lock,
  "git-branch": GitBranch,
};

function getIcon(name?: string) {
  if (!name) return FileText;
  return iconMap[name] || FileText;
}

function SidebarLink({ item, depth = 0 }: { item: SidebarItem; depth?: number }) {
  const pathname = usePathname();
  const isActive = pathname === item.href;
  const [expanded, setExpanded] = useState(
    item.children?.some((c) => pathname === c.href || pathname.startsWith(c.href + "/")) || false
  );
  const Icon = getIcon(item.icon);

  if (item.children && item.children.length > 0) {
    return (
      <div>
        <button
          onClick={() => setExpanded(!expanded)}
          className={cn(
            "flex w-full items-center gap-2 rounded-lg px-3 py-2 text-sm transition-colors",
            "text-[rgb(var(--text-secondary))] hover:bg-[var(--overlay-subtle)] hover:text-[rgb(var(--text-primary))]"
          )}
          style={{ paddingLeft: `${12 + depth * 12}px` }}
        >
          {expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
          <span className="font-medium capitalize">{item.title.replace(/-/g, " ")}</span>
        </button>
        {expanded && (
          <div className="mt-0.5">
            {item.children.map((child) => (
              <SidebarLink key={child.href} item={child} depth={depth + 1} />
            ))}
          </div>
        )}
      </div>
    );
  }

  return (
    <Link
      href={item.href}
      className={cn(
        "flex items-center gap-2 rounded-lg px-3 py-2 text-sm transition-colors",
        isActive
          ? "bg-blue-500/10 text-blue-400 font-medium"
          : "text-[rgb(var(--text-secondary))] hover:bg-[var(--overlay-subtle)] hover:text-[rgb(var(--text-primary))]"
      )}
      style={{ paddingLeft: `${12 + depth * 12}px` }}
    >
      <Icon size={14} className={isActive ? "text-blue-400" : "text-[rgb(var(--text-muted))]"} />
      <span>{item.title}</span>
      {item.badge && (
        <span className="ml-auto rounded-full bg-emerald-500/10 px-1.5 py-0.5 text-[10px] font-bold text-emerald-400">
          {item.badge}
        </span>
      )}
    </Link>
  );
}

interface DocsSidebarProps {
  sections: SidebarSection[];
  mobileOpen?: boolean;
  onMobileClose?: () => void;
}

export function DocsSidebar({ sections, mobileOpen, onMobileClose }: DocsSidebarProps) {
  const t = useTranslations("docs");

  const sectionLabels: Record<string, string> = {
    business: t("businessCenter"),
    technical: t("technicalDocs"),
  };

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const sectionIcons: Record<string, React.ComponentType<any>> = {
    business: TrendingUp,
    technical: Wrench,
  };

  const sidebar = (
    <nav className="flex h-full flex-col overflow-y-auto pb-8">
      <div className="sticky top-0 z-10 flex items-center justify-between bg-[rgb(var(--bg-primary))]/80 px-4 py-4 backdrop-blur-sm md:hidden">
        <span className="text-sm font-bold">{t("title")}</span>
        <button onClick={onMobileClose} className="rounded-lg p-1 hover:bg-[var(--overlay-subtle)]">
          <X size={18} />
        </button>
      </div>
      <div className="space-y-6 px-2">
        {sections.map((section) => {
          const SIcon = sectionIcons[section.category] || FileText;
          return (
            <div key={section.category}>
              <div className="mb-2 flex items-center gap-2 px-3">
                <SIcon size={14} className="text-[rgb(var(--text-muted))]" />
                <span className="text-xs font-bold uppercase tracking-wider text-[rgb(var(--text-muted))]">
                  {sectionLabels[section.category] || section.title}
                </span>
              </div>
              <div className="space-y-0.5">
                {section.items.map((item) => (
                  <SidebarLink key={item.href} item={item} />
                ))}
              </div>
            </div>
          );
        })}
      </div>
    </nav>
  );

  return (
    <>
      {/* Desktop sidebar */}
      <aside className="hidden w-64 shrink-0 border-r border-[var(--glass-border)] md:block">
        <div className="sticky top-16 h-[calc(100vh-4rem)] overflow-y-auto">
          {sidebar}
        </div>
      </aside>

      {/* Mobile sidebar overlay */}
      {mobileOpen && (
        <div className="fixed inset-0 z-50 md:hidden">
          <div className="absolute inset-0 bg-black/50" onClick={onMobileClose} />
          <aside className="absolute inset-y-0 left-0 w-72 bg-[rgb(var(--bg-primary))] shadow-2xl">
            {sidebar}
          </aside>
        </div>
      )}
    </>
  );
}
