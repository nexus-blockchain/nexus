"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { useEntityStore } from "@/stores/entity";
import { useShopActions } from "@/hooks/useShop";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { TxButton } from "@/components/shared/TxButton";
import { SHOP_TYPES } from "@/lib/constants";
import { Store, ArrowLeft } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

export default function CreateShopPage() {
  const router = useRouter();
  const { currentEntityId } = useEntityStore();
  const actions = useShopActions();
  const t = useTranslations("shops");
  const tc = useTranslations("common");

  const [name, setName] = useState("");
  const [shopType, setShopType] = useState<string>("Online");
  const [initialFund, setInitialFund] = useState("");
  const [lat, setLat] = useState("");
  const [lng, setLng] = useState("");

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  const handleCreate = async () => {
    if (!name.trim()) return;
    const fund = initialFund ? BigInt(initialFund) : BigInt(0);
    await actions.createShop(currentEntityId, name, shopType, fund);
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/shops"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div>
          <h1 className="text-3xl font-bold tracking-tight">{t("createShop")}</h1>
          <p className="text-muted-foreground">Set up a new shop under your entity</p>
        </div>
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><Store className="h-5 w-5" />Shop Details</CardTitle>
            <CardDescription>Basic information for your new shop</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">Shop Name *</label>
              <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="Enter shop name" />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Shop Type</label>
              <Select value={shopType} onChange={(e) => setShopType(e.target.value)}>
                {SHOP_TYPES.map((type) => (
                  <option key={type} value={type}>{type}</option>
                ))}
              </Select>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Initial Fund (NEX)</label>
              <Input type="number" value={initialFund} onChange={(e) => setInitialFund(e.target.value)} placeholder="0" min="0" />
              <p className="text-xs text-muted-foreground">Operating fund to cover transaction fees</p>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Location (Optional)</CardTitle>
            <CardDescription>Set a physical location for your shop</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="grid gap-4 grid-cols-2">
              <div className="space-y-2">
                <label className="text-sm font-medium">Latitude</label>
                <Input type="number" value={lat} onChange={(e) => setLat(e.target.value)} placeholder="0.000000" step="0.000001" />
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">Longitude</label>
                <Input type="number" value={lng} onChange={(e) => setLng(e.target.value)} placeholder="0.000000" step="0.000001" />
              </div>
            </div>
            <p className="text-xs text-muted-foreground">Location can also be set later from Shop settings</p>
          </CardContent>
        </Card>
      </div>

      <div className="flex items-center gap-4">
        <TxButton onClick={handleCreate} txStatus={actions.txState.status} disabled={!name.trim()}>
          <Store className="mr-2 h-4 w-4" />{t("createShop")}
        </TxButton>
        <Button variant="outline" asChild>
          <Link href="/shops">{tc("cancel")}</Link>
        </Button>
      </div>

      {actions.txState.status === "finalized" && (
        <div className="rounded-lg border border-green-200 bg-green-50 p-4">
          <p className="text-sm text-green-800">Shop created successfully!</p>
          <Button variant="link" className="mt-1 h-auto p-0 text-green-700" onClick={() => router.push("/shops")}>
            Go to Shops
          </Button>
        </div>
      )}
      {actions.txState.status === "error" && (
        <p className="text-sm text-destructive">{actions.txState.error}</p>
      )}
    </div>
  );
}
