"use client";

import { useEntityStore } from "@/stores/entity";
import { useShops, useShopActions } from "@/hooks/useShop";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { Store, Plus, MapPin, Pause, Play } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

export default function ShopListPage() {
  const { currentEntityId } = useEntityStore();
  const { shops, isLoading } = useShops(currentEntityId);
  const shopActions = useShopActions();
  const t = useTranslations("shops");
  const tc = useTranslations("common");

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">{t("title")}</h1>
          <p className="text-muted-foreground">{t("subtitle")}</p>
        </div>
        <Button asChild>
          <Link href="/shops/create"><Plus className="mr-2 h-4 w-4" />{t("createShop")}</Link>
        </Button>
      </div>

      {isLoading ? (
        <div className="flex items-center justify-center py-12"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>
      ) : shops.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Store className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">{t("noShops")}</p>
            <p className="text-sm text-muted-foreground">{t("noShopsDesc")}</p>
          </CardContent>
        </Card>
      ) : (
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {shops.map((shop) => (
            <Card key={shop.id} className="hover:shadow-md transition-shadow">
              <CardHeader className="pb-3">
                <div className="flex items-center justify-between">
                  <CardTitle className="text-lg">{shop.name}</CardTitle>
                  <StatusBadge status={shop.status} />
                </div>
              </CardHeader>
              <CardContent className="space-y-3">
                <div className="flex items-center gap-2 text-sm text-muted-foreground">
                  <Store className="h-4 w-4" />
                  <span>ID: {shop.id}</span>
                </div>
                <div className="flex items-center gap-2 text-sm text-muted-foreground">
                  <MapPin className="h-4 w-4" />
                  <span>{shop.location ? `${shop.location.lat}, ${shop.location.lng}` : t("noLocation")}</span>
                </div>
                <div className="flex items-center gap-2 text-sm">
                  <span className="text-muted-foreground">Owner:</span>
                  <AddressDisplay address={shop.managers?.[0] || ""} chars={4} />
                </div>
                <div className="flex gap-2 pt-2">
                  <Button variant="outline" size="sm" asChild className="flex-1">
                    <Link href={`/shops/${shop.id}`}>Manage</Link>
                  </Button>
                  {shop.status === "Active" ? (
                    <Button variant="ghost" size="icon" onClick={() => shopActions.pauseShop(shop.id)} title="Pause">
                      <Pause className="h-4 w-4" />
                    </Button>
                  ) : shop.status === "Paused" ? (
                    <Button variant="ghost" size="icon" onClick={() => shopActions.resumeShop(shop.id)} title="Resume">
                      <Play className="h-4 w-4" />
                    </Button>
                  ) : null}
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}
