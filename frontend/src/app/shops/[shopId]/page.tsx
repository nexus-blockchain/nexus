"use client";

import { use } from "react";
import { useEntityStore } from "@/stores/entity";
import { useShops, useShopActions } from "@/hooks/useShop";
import { useProducts } from "@/hooks/useProducts";
import { useOrders } from "@/hooks/useOrder";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { formatBalance, formatNumber } from "@/lib/utils";
import {
  Store, Package, ShoppingCart, Star, Settings, MapPin, ArrowLeft,
  Pause, Play, Users, Gift, Edit,
} from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

export default function ShopDetailPage({ params }: { params: Promise<{ shopId: string }> }) {
  const { shopId: shopIdStr } = use(params);
  const shopId = Number(shopIdStr);
  const { currentEntityId } = useEntityStore();
  const { shops, isLoading } = useShops(currentEntityId);
  const { products } = useProducts(shopId);
  const { orders } = useOrders(shopId);
  const shopActions = useShopActions();
  const tc = useTranslations("common");

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  const shop = shops.find((s) => s.id === shopId);

  if (isLoading) {
    return <div className="flex h-full items-center justify-center"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>;
  }

  if (!shop) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4">
        <Store className="h-16 w-16 text-muted-foreground/50" />
        <p className="text-muted-foreground">Shop not found</p>
        <Button variant="outline" asChild><Link href="/shops">Back to Shops</Link></Button>
      </div>
    );
  }

  const pendingOrders = orders.filter((o) => o.status === "Pending" || o.status === "Paid").length;
  const avgRating = shop.ratingCount > 0 ? (shop.ratingTotal / shop.ratingCount).toFixed(1) : "—";

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/shops"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <div className="flex items-center gap-3">
            <h1 className="text-3xl font-bold tracking-tight">{shop.name}</h1>
            <StatusBadge status={shop.status} />
            {shop.isPrimary && <Badge variant="outline">Primary</Badge>}
          </div>
          <p className="text-muted-foreground">Shop #{shop.id} &middot; {shop.shopType}</p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" asChild>
            <Link href={`/shops/${shopId}/edit`}><Edit className="mr-2 h-4 w-4" />Edit</Link>
          </Button>
          {shop.status === "Active" ? (
            <Button variant="outline" onClick={() => shopActions.pauseShop(shopId)}>
              <Pause className="mr-2 h-4 w-4" />Pause
            </Button>
          ) : shop.status === "Paused" ? (
            <Button variant="outline" onClick={() => shopActions.resumeShop(shopId)}>
              <Play className="mr-2 h-4 w-4" />Resume
            </Button>
          ) : null}
        </div>
      </div>

      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-5">
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Products</CardTitle>
            <Package className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{products.length}</div>
            <p className="text-xs text-muted-foreground">{products.filter((p) => p.status === "Active").length} active</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Total Orders</CardTitle>
            <ShoppingCart className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{formatNumber(shop.totalOrders)}</div>
            <p className="text-xs text-muted-foreground">{pendingOrders} pending</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Total Sales</CardTitle>
            <Store className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{formatBalance(shop.totalSales)} NEX</div>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Rating</CardTitle>
            <Star className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{avgRating}</div>
            <p className="text-xs text-muted-foreground">{shop.ratingCount} reviews</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Location</CardTitle>
            <MapPin className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            {shop.location ? (
              <p className="text-sm font-mono">{shop.location.lat}, {shop.location.lng}</p>
            ) : (
              <p className="text-sm text-muted-foreground">Not set</p>
            )}
          </CardContent>
        </Card>
      </div>

      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
        <Link href={`/shops/${shopId}/products`} className="group">
          <Card className="transition-shadow hover:shadow-md">
            <CardHeader>
              <CardTitle className="flex items-center gap-2"><Package className="h-5 w-5" />Products</CardTitle>
              <CardDescription>Manage products and services</CardDescription>
            </CardHeader>
            <CardContent>
              <p className="text-sm text-muted-foreground">{products.length} products &middot; {products.filter((p) => p.status === "Active").length} active</p>
            </CardContent>
          </Card>
        </Link>
        <Link href={`/shops/${shopId}/orders`} className="group">
          <Card className="transition-shadow hover:shadow-md">
            <CardHeader>
              <CardTitle className="flex items-center gap-2"><ShoppingCart className="h-5 w-5" />Orders</CardTitle>
              <CardDescription>View and manage orders</CardDescription>
            </CardHeader>
            <CardContent>
              <p className="text-sm text-muted-foreground">{formatNumber(shop.totalOrders)} total &middot; {pendingOrders} pending</p>
            </CardContent>
          </Card>
        </Link>
        <Link href={`/shops/${shopId}/reviews`} className="group">
          <Card className="transition-shadow hover:shadow-md">
            <CardHeader>
              <CardTitle className="flex items-center gap-2"><Star className="h-5 w-5" />Reviews</CardTitle>
              <CardDescription>Customer ratings and feedback</CardDescription>
            </CardHeader>
            <CardContent>
              <p className="text-sm text-muted-foreground">{avgRating} avg &middot; {shop.ratingCount} reviews</p>
            </CardContent>
          </Card>
        </Link>
        <Link href={`/shops/${shopId}/points`} className="group">
          <Card className="transition-shadow hover:shadow-md">
            <CardHeader>
              <CardTitle className="flex items-center gap-2"><Gift className="h-5 w-5" />Points System</CardTitle>
              <CardDescription>Loyalty points configuration</CardDescription>
            </CardHeader>
            <CardContent>
              <p className="text-sm text-muted-foreground">Configure shop loyalty rewards</p>
            </CardContent>
          </Card>
        </Link>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2"><Users className="h-5 w-5" />Managers</CardTitle>
        </CardHeader>
        <CardContent>
          {shop.managers.length === 0 ? (
            <p className="text-sm text-muted-foreground">No managers assigned</p>
          ) : (
            <div className="flex flex-wrap gap-3">
              {shop.managers.map((addr) => (
                <div key={addr} className="flex items-center gap-2 rounded-lg border px-3 py-2">
                  <AddressDisplay address={addr} />
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
