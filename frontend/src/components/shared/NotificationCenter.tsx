"use client";

import { useState, useEffect, useRef } from "react";
import { Bell, X, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useEntityEvents, type EntityEvent } from "@/hooks/useEntityEvents";
import { useEntityStore } from "@/stores/entity";

const SECTION_LABELS: Record<string, string> = {
  entityRegistry: "Entity",
  entityShop: "Shop",
  entityProduct: "Product",
  entityOrder: "Order",
  entityReview: "Review",
  entityToken: "Token",
  entityMarket: "Market",
  entityMember: "Member",
  entityGovernance: "Governance",
  entityDisclosure: "Disclosure",
  entityKyc: "KYC",
  entityTokensale: "Token Sale",
  commissionCore: "Commission",
  nexMarket: "NEX Market",
  storageService: "Storage",
  storageLifecycle: "Storage Lifecycle",
  grouprobotRegistry: "Robot",
  grouprobotCommunity: "Community",
  grouprobotConsensus: "Consensus",
  grouprobotSubscription: "Subscription",
  grouprobotRewards: "Rewards",
  disputeArbitration: "Arbitration",
  disputeEscrow: "Escrow",
  disputeEvidence: "Evidence",
  adsCore: "Ads",
  adsEntity: "Ad Placement",
  adsGrouprobot: "Community Ads",
};

const SECTION_COLORS: Record<string, string> = {
  entity: "bg-blue-500",
  shop: "bg-green-500",
  token: "bg-yellow-500",
  market: "bg-purple-500",
  member: "bg-cyan-500",
  commission: "bg-orange-500",
  governance: "bg-indigo-500",
  nex: "bg-violet-500",
  storage: "bg-teal-500",
  grouprobot: "bg-pink-500",
  dispute: "bg-red-500",
  ads: "bg-amber-500",
};

function getSectionColor(section: string): string {
  const lower = section.toLowerCase();
  for (const [key, color] of Object.entries(SECTION_COLORS)) {
    if (lower.includes(key)) return color;
  }
  return "bg-primary";
}

function formatEventName(section: string, method: string): string {
  const label = SECTION_LABELS[section] || section.replace(/([A-Z])/g, " $1").trim();
  return `${label}: ${method.replace(/([A-Z])/g, " $1").trim()}`;
}

function timeAgo(timestamp: number): string {
  const seconds = Math.floor((Date.now() - timestamp) / 1000);
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  return `${Math.floor(hours / 24)}d ago`;
}

function EventItem({ event }: { event: EntityEvent }) {
  return (
    <div className="flex items-start gap-3 rounded-md px-3 py-2 hover:bg-muted/50 transition-colors">
      <div className={`mt-1 h-2 w-2 rounded-full shrink-0 ${getSectionColor(event.section)}`} />
      <div className="min-w-0 flex-1">
        <p className="text-sm font-medium truncate">{formatEventName(event.section, event.method)}</p>
        <p className="text-xs text-muted-foreground">{timeAgo(event.timestamp)}</p>
      </div>
    </div>
  );
}

export function NotificationCenter() {
  const [open, setOpen] = useState(false);
  const [seenCount, setSeenCount] = useState(0);
  const { currentEntityId } = useEntityStore();
  const { events, clearEvents } = useEntityEvents(currentEntityId);
  const bellRef = useRef<HTMLButtonElement>(null);

  const unreadCount = Math.max(0, events.length - seenCount);

  useEffect(() => {
    if (open) setSeenCount(events.length);
  }, [open, events.length]);

  return (
    <div className="relative">
      <Button
        ref={bellRef}
        variant="ghost"
        size="icon"
        className="relative"
        onClick={() => setOpen(!open)}
      >
        <Bell className="h-5 w-5" />
        {unreadCount > 0 && (
          <span className="absolute -top-0.5 -right-0.5 flex h-4 min-w-4 items-center justify-center rounded-full bg-destructive px-1 text-[10px] font-bold text-destructive-foreground">
            {unreadCount > 99 ? "99+" : unreadCount}
          </span>
        )}
      </Button>

      {open && (
        <>
          <div className="fixed inset-0 z-40" onClick={() => setOpen(false)} />
          <div className="absolute right-0 top-full z-50 mt-2 w-80 rounded-lg border bg-popover shadow-lg sm:w-96">
            <div className="flex items-center justify-between border-b px-4 py-3">
              <h3 className="text-sm font-semibold">
                Notifications
                {events.length > 0 && (
                  <span className="ml-1.5 text-xs font-normal text-muted-foreground">({events.length})</span>
                )}
              </h3>
              <div className="flex items-center gap-1">
                {events.length > 0 && (
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-7 w-7"
                    onClick={() => { clearEvents(); setSeenCount(0); }}
                  >
                    <Trash2 className="h-3.5 w-3.5" />
                  </Button>
                )}
                <Button variant="ghost" size="icon" className="h-7 w-7" onClick={() => setOpen(false)}>
                  <X className="h-3.5 w-3.5" />
                </Button>
              </div>
            </div>
            <div className="max-h-96 overflow-y-auto">
              {events.length === 0 ? (
                <div className="px-4 py-8 text-center">
                  <Bell className="mx-auto h-8 w-8 text-muted-foreground/50" />
                  <p className="mt-2 text-sm text-muted-foreground">No notifications yet</p>
                  <p className="mt-1 text-xs text-muted-foreground/60">Chain events will appear here in real time</p>
                </div>
              ) : (
                <div className="py-1">
                  {events.map((event) => (
                    <EventItem key={event.id} event={event} />
                  ))}
                </div>
              )}
            </div>
          </div>
        </>
      )}
    </div>
  );
}
