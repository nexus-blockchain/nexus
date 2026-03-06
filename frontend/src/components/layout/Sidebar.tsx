"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { cn } from "@/lib/utils";
import { useUiStore } from "@/stores/ui";
import { useTranslations } from "next-intl";
import {
  LayoutDashboard,
  Building2,
  Store,
  Coins,
  TrendingUp,
  Users,
  Wallet,
  Vote,
  FileText,
  ShieldCheck,
  Rocket,
  ChevronLeft,
  ChevronRight,
  ArrowLeftRight,
  HardDrive,
  Bot,
  Scale,
  Megaphone,
  type LucideIcon,
} from "lucide-react";

interface NavChild {
  href: string;
  labelKey: string;
}
interface NavEntry {
  href?: string;
  labelKey: string;
  icon: LucideIcon;
  children?: NavChild[];
}
interface NavSection {
  sectionKey?: string;
  items: NavEntry[];
}

const navSections: NavSection[] = [
  {
    items: [
      { href: "/", labelKey: "dashboard", icon: LayoutDashboard },
    ],
  },
  {
    sectionKey: "sectionEntity",
    items: [
      {
        labelKey: "entity",
        icon: Building2,
        children: [
          { href: "/entity/create", labelKey: "createEntity" },
          { href: "/entity/settings", labelKey: "settings" },
          { href: "/entity/admins", labelKey: "admins" },
          { href: "/entity/fund", labelKey: "fund" },
        ],
      },
      {
        labelKey: "shops",
        icon: Store,
        children: [
          { href: "/shops", labelKey: "shops" },
          { href: "/shops/create", labelKey: "createShop" },
        ],
      },
      {
        labelKey: "token",
        icon: Coins,
        children: [
          { href: "/token/config", labelKey: "tokenConfig" },
          { href: "/token/holders", labelKey: "holders" },
          { href: "/token/dividend", labelKey: "dividend" },
          { href: "/token/lock", labelKey: "lock" },
          { href: "/token/transfer", labelKey: "transfer" },
        ],
      },
      {
        labelKey: "market",
        icon: TrendingUp,
        children: [
          { href: "/market", labelKey: "market" },
          { href: "/market/orders", labelKey: "myOrders" },
          { href: "/market/settings", labelKey: "settings" },
        ],
      },
      {
        labelKey: "members",
        icon: Users,
        children: [
          { href: "/members", labelKey: "members" },
          { href: "/members/levels", labelKey: "levels" },
          { href: "/members/rules", labelKey: "rules" },
          { href: "/members/pending", labelKey: "pending" },
          { href: "/members/policy", labelKey: "policy" },
        ],
      },
      {
        labelKey: "commission",
        icon: Wallet,
        children: [
          { href: "/commission", labelKey: "commission" },
          { href: "/commission/config", labelKey: "config" },
          { href: "/commission/withdraw", labelKey: "withdraw" },
          { href: "/commission/pool", labelKey: "pool" },
        ],
      },
      {
        labelKey: "governance",
        icon: Vote,
        children: [
          { href: "/governance", labelKey: "governance" },
          { href: "/governance/config", labelKey: "config" },
        ],
      },
      {
        labelKey: "disclosure",
        icon: FileText,
        children: [
          { href: "/disclosure", labelKey: "disclosure" },
          { href: "/disclosure/insiders", labelKey: "insiders" },
        ],
      },
      {
        labelKey: "kyc",
        icon: ShieldCheck,
        children: [
          { href: "/kyc", labelKey: "kyc" },
          { href: "/kyc/settings", labelKey: "settings" },
          { href: "/kyc/providers", labelKey: "providers" },
        ],
      },
      {
        labelKey: "tokensale",
        icon: Rocket,
        children: [
          { href: "/tokensale", labelKey: "tokensale" },
          { href: "/tokensale/create", labelKey: "createRound" },
        ],
      },
    ],
  },
  {
    sectionKey: "sectionTrading",
    items: [
      {
        labelKey: "trading",
        icon: ArrowLeftRight,
        children: [
          { href: "/trading", labelKey: "tradingOrderbook" },
          { href: "/trading/my-orders", labelKey: "tradingMyOrders" },
          { href: "/trading/my-trades", labelKey: "tradingMyTrades" },
          { href: "/trading/disputes", labelKey: "tradingDisputes" },
        ],
      },
    ],
  },
  {
    sectionKey: "sectionStorage",
    items: [
      {
        labelKey: "storage",
        icon: HardDrive,
        children: [
          { href: "/storage", labelKey: "storagePins" },
          { href: "/storage/operators", labelKey: "storageOperators" },
          { href: "/storage/billing", labelKey: "storageBilling" },
        ],
      },
    ],
  },
  {
    sectionKey: "sectionRobot",
    items: [
      {
        labelKey: "robot",
        icon: Bot,
        children: [
          { href: "/robot", labelKey: "robotBots" },
          { href: "/robot/operators", labelKey: "robotOperators" },
          { href: "/robot/communities", labelKey: "robotCommunities" },
          { href: "/robot/nodes", labelKey: "robotNodes" },
          { href: "/robot/rewards", labelKey: "robotRewards" },
          { href: "/robot/subscriptions", labelKey: "robotSubscriptions" },
        ],
      },
    ],
  },
  {
    sectionKey: "sectionDispute",
    items: [
      {
        labelKey: "dispute",
        icon: Scale,
        children: [
          { href: "/dispute", labelKey: "disputeComplaints" },
          { href: "/dispute/escrow", labelKey: "disputeEscrow" },
          { href: "/dispute/evidence", labelKey: "disputeEvidence" },
        ],
      },
    ],
  },
  {
    sectionKey: "sectionAds",
    items: [
      {
        labelKey: "ads",
        icon: Megaphone,
        children: [
          { href: "/ads", labelKey: "adsCampaigns" },
          { href: "/ads/placements", labelKey: "adsPlacements" },
          { href: "/ads/staking", labelKey: "adsStaking" },
          { href: "/ads/revenue", labelKey: "adsRevenue" },
        ],
      },
    ],
  },
];

interface NavItemProps {
  item: NavEntry;
  collapsed: boolean;
  pathname: string;
  t: (key: string) => string;
}

function NavItem({ item, collapsed, pathname, t }: NavItemProps) {
  const Icon = item.icon;

  if (item.href) {
    const isActive = pathname === item.href;
    return (
      <Link
        href={item.href}
        className={cn(
          "flex items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors",
          isActive
            ? "bg-sidebar-accent text-sidebar-accent-foreground font-medium"
            : "text-sidebar-foreground/70 hover:bg-sidebar-accent/50 hover:text-sidebar-foreground"
        )}
      >
        <Icon className="h-4 w-4 shrink-0" />
        {!collapsed && <span>{t(item.labelKey)}</span>}
      </Link>
    );
  }

  const children = item.children || [];
  const isGroupActive = children.some(
    (c) => pathname === c.href || pathname.startsWith(c.href + "/")
  );

  return (
    <div className="space-y-0.5">
      <div
        className={cn(
          "flex items-center gap-3 rounded-lg px-3 py-1.5 text-xs font-semibold uppercase tracking-wider",
          isGroupActive ? "text-sidebar-primary" : "text-sidebar-foreground/50"
        )}
      >
        <Icon className="h-4 w-4 shrink-0" />
        {!collapsed && <span>{t(item.labelKey)}</span>}
      </div>
      {!collapsed &&
        children.map((child) => {
          const isActive =
            pathname === child.href ||
            pathname.startsWith(child.href + "/");
          return (
            <Link
              key={child.href}
              href={child.href}
              className={cn(
                "flex items-center gap-3 rounded-lg py-1.5 pl-10 pr-3 text-sm transition-colors",
                isActive
                  ? "bg-sidebar-accent text-sidebar-accent-foreground font-medium"
                  : "text-sidebar-foreground/60 hover:bg-sidebar-accent/50 hover:text-sidebar-foreground"
              )}
            >
              {t(child.labelKey)}
            </Link>
          );
        })}
    </div>
  );
}

function SidebarContent({ collapsed, pathname, t, onNavigate }: { collapsed: boolean; pathname: string; t: (key: string) => string; onNavigate?: () => void }) {
  return (
    <nav className="flex-1 space-y-1 overflow-y-auto p-3" onClick={onNavigate}>
      {navSections.map((section, si) => (
        <div key={si}>
          {section.sectionKey && !collapsed && (
            <div className="mb-1 mt-4 border-t pt-3 px-3">
              <span className="text-[10px] font-bold uppercase tracking-widest text-sidebar-foreground/30">
                {t(section.sectionKey)}
              </span>
            </div>
          )}
          {section.sectionKey && collapsed && (
            <div className="my-2 border-t" />
          )}
          {section.items.map((item, ii) => (
            <NavItem
              key={ii}
              item={item}
              collapsed={collapsed}
              pathname={pathname}
              t={t}
            />
          ))}
        </div>
      ))}
    </nav>
  );
}

export function Sidebar() {
  const pathname = usePathname();
  const { sidebarCollapsed, toggleSidebar, mobileSidebarOpen, setMobileSidebarOpen } = useUiStore();
  const t = useTranslations("nav");

  return (
    <>
      {/* Desktop sidebar */}
      <aside
        className={cn(
          "hidden md:flex h-screen flex-col border-r bg-sidebar transition-all duration-300",
          sidebarCollapsed ? "w-16" : "w-64"
        )}
      >
        <div className="flex h-14 items-center justify-between border-b px-4">
          {!sidebarCollapsed && (
            <span className="text-lg font-bold text-sidebar-primary">NEXUS</span>
          )}
          <button
            onClick={toggleSidebar}
            className="rounded-md p-1.5 text-sidebar-foreground/50 hover:bg-sidebar-accent hover:text-sidebar-foreground"
          >
            {sidebarCollapsed ? (
              <ChevronRight className="h-4 w-4" />
            ) : (
              <ChevronLeft className="h-4 w-4" />
            )}
          </button>
        </div>
        <SidebarContent collapsed={sidebarCollapsed} pathname={pathname} t={t} />
      </aside>

      {/* Mobile sidebar overlay */}
      {mobileSidebarOpen && (
        <div className="fixed inset-0 z-50 md:hidden">
          <div className="fixed inset-0 bg-black/50" onClick={() => setMobileSidebarOpen(false)} />
          <aside className="fixed inset-y-0 left-0 z-50 flex w-72 flex-col bg-sidebar shadow-xl">
            <div className="flex h-14 items-center justify-between border-b px-4">
              <span className="text-lg font-bold text-sidebar-primary">NEXUS</span>
              <button
                onClick={() => setMobileSidebarOpen(false)}
                className="rounded-md p-1.5 text-sidebar-foreground/50 hover:bg-sidebar-accent hover:text-sidebar-foreground"
              >
                <ChevronLeft className="h-4 w-4" />
              </button>
            </div>
            <SidebarContent
              collapsed={false}
              pathname={pathname}
              t={t}
              onNavigate={() => setMobileSidebarOpen(false)}
            />
          </aside>
        </div>
      )}
    </>
  );
}
