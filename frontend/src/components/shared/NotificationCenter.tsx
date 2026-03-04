"use client";

import { useState } from "react";
import { Bell, X, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useEntityEvents, type EntityEvent } from "@/hooks/useEntityEvents";
import { useEntityStore } from "@/stores/entity";

function formatEventName(section: string, method: string): string {
  const cleanSection = section.replace(/^entity/i, "").replace(/^commission/i, "Commission: ");
  return `${cleanSection}.${method}`;
}

function timeAgo(timestamp: number): string {
  const seconds = Math.floor((Date.now() - timestamp) / 1000);
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  return `${hours}h ago`;
}

function EventItem({ event }: { event: EntityEvent }) {
  return (
    <div className="flex items-start gap-3 rounded-md px-3 py-2 hover:bg-muted/50 transition-colors">
      <div className="mt-0.5 h-2 w-2 rounded-full bg-primary shrink-0" />
      <div className="min-w-0 flex-1">
        <p className="text-sm font-medium truncate">{formatEventName(event.section, event.method)}</p>
        <p className="text-xs text-muted-foreground">{timeAgo(event.timestamp)}</p>
      </div>
    </div>
  );
}

export function NotificationCenter() {
  const [open, setOpen] = useState(false);
  const { currentEntityId } = useEntityStore();
  const { events, clearEvents } = useEntityEvents(currentEntityId);

  const unreadCount = events.length;

  return (
    <div className="relative">
      <Button
        variant="ghost"
        size="icon"
        className="relative"
        onClick={() => setOpen(!open)}
      >
        <Bell className="h-5 w-5" />
        {unreadCount > 0 && (
          <span className="absolute -top-0.5 -right-0.5 flex h-4 w-4 items-center justify-center rounded-full bg-destructive text-[10px] font-bold text-destructive-foreground">
            {unreadCount > 9 ? "9+" : unreadCount}
          </span>
        )}
      </Button>

      {open && (
        <>
          <div className="fixed inset-0 z-40" onClick={() => setOpen(false)} />
          <div className="absolute right-0 top-full z-50 mt-2 w-80 rounded-lg border bg-popover shadow-lg">
            <div className="flex items-center justify-between border-b px-4 py-3">
              <h3 className="text-sm font-semibold">Notifications</h3>
              <div className="flex items-center gap-1">
                {events.length > 0 && (
                  <Button variant="ghost" size="icon" className="h-7 w-7" onClick={clearEvents} title="Clear all">
                    <Trash2 className="h-3.5 w-3.5" />
                  </Button>
                )}
                <Button variant="ghost" size="icon" className="h-7 w-7" onClick={() => setOpen(false)}>
                  <X className="h-3.5 w-3.5" />
                </Button>
              </div>
            </div>
            <div className="max-h-80 overflow-y-auto">
              {events.length === 0 ? (
                <div className="px-4 py-8 text-center">
                  <Bell className="mx-auto h-8 w-8 text-muted-foreground/50" />
                  <p className="mt-2 text-sm text-muted-foreground">No notifications yet</p>
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
