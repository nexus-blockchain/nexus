"use client";

import { useEntityStore } from "@/stores/entity";
import { useEntity } from "@/hooks/useEntity";
import { useShops } from "@/hooks/useShop";
import { useToken } from "@/hooks/useToken";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { formatBalance, formatNumber } from "@/lib/utils";
import {
  Building2,
  Store,
  Users,
  ShoppingCart,
  Coins,
  Wallet,
  TrendingUp,
  Activity,
} from "lucide-react";

export default function DashboardPage() {
  const { currentEntityId } = useEntityStore();
  const { data: entity, isLoading } = useEntity(currentEntityId);
  const { shops } = useShops(currentEntityId);
  const { config: tokenConfig } = useToken(currentEntityId);

  if (!currentEntityId) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-center">
          <Building2 className="mx-auto h-16 w-16 text-muted-foreground/50" />
          <h2 className="mt-4 text-xl font-semibold">No Entity Selected</h2>
          <p className="mt-2 text-muted-foreground">
            Select an entity from the header dropdown to get started.
          </p>
        </div>
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
      </div>
    );
  }

  if (!entity) {
    return (
      <div className="flex h-full items-center justify-center">
        <p className="text-muted-foreground">Entity not found</p>
      </div>
    );
  }

  const fundHealthPercent = 75;

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">{entity.name}</h1>
        <p className="text-muted-foreground">Entity Dashboard</p>
      </div>

      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-5">
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Status</CardTitle>
            <Activity className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <StatusBadge status={entity.status} />
            <p className="mt-1 text-xs text-muted-foreground">{entity.entityType}</p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Shops</CardTitle>
            <Store className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{shops.length}</div>
            <p className="text-xs text-muted-foreground">
              {shops.filter((s) => s.status === "Active").length} active
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Total Orders</CardTitle>
            <ShoppingCart className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{formatNumber(entity.totalOrders)}</div>
            <p className="text-xs text-muted-foreground">all time</p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Total Sales</CardTitle>
            <TrendingUp className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{formatBalance(entity.totalSales)} NEX</div>
            <p className="text-xs text-muted-foreground">all time</p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Token</CardTitle>
            <Coins className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{tokenConfig ? tokenConfig.tokenType : "N/A"}</div>
            <p className="text-xs text-muted-foreground">
              {tokenConfig?.enabled ? "Active" : "Not configured"}
            </p>
          </CardContent>
        </Card>
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Wallet className="h-5 w-5" />
              Operating Fund
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between">
              <span className="text-sm text-muted-foreground">Balance</span>
              <span className="text-lg font-semibold">-- NEX</span>
            </div>
            <Progress value={fundHealthPercent} className="h-2" />
            <div className="flex items-center justify-between text-xs text-muted-foreground">
              <span>Min: 100 NEX</span>
              <span>Warning: 500 NEX</span>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Activity className="h-5 w-5" />
              Entity Details
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-3">
              <div className="flex items-center justify-between">
                <span className="text-sm text-muted-foreground">Type</span>
                <span className="text-sm font-medium">{entity.entityType}</span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-sm text-muted-foreground">Governance</span>
                <span className="text-sm font-medium">{entity.governanceMode}</span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-sm text-muted-foreground">Verified</span>
                <StatusBadge status={entity.verified ? "Verified" : "Unverified"} />
              </div>
              <div className="flex items-center justify-between">
                <span className="text-sm text-muted-foreground">Admins</span>
                <span className="text-sm font-medium">{entity.admins.length}</span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-sm text-muted-foreground">Primary Shop</span>
                <span className="text-sm font-medium">#{entity.primaryShopId}</span>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
