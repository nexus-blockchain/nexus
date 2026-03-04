"use client";

import { use, useState, useEffect } from "react";
import { useEntityStore } from "@/stores/entity";
import { useShops, useShopActions } from "@/hooks/useShop";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { TxButton } from "@/components/shared/TxButton";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { ArrowLeft, Save, MapPin, UserPlus, Trash2 } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

export default function ShopEditPage({ params }: { params: Promise<{ shopId: string }> }) {
  const { shopId: shopIdStr } = use(params);
  const shopId = Number(shopIdStr);
  const { currentEntityId } = useEntityStore();
  const { shops, isLoading } = useShops(currentEntityId);
  const actions = useShopActions();
  const tc = useTranslations("common");

  const shop = shops.find((s) => s.id === shopId);

  const [name, setName] = useState("");
  const [logoCid, setLogoCid] = useState("");
  const [descriptionCid, setDescriptionCid] = useState("");
  const [lat, setLat] = useState("");
  const [lng, setLng] = useState("");
  const [newManager, setNewManager] = useState("");

  useEffect(() => {
    if (shop) {
      setName(shop.name);
      setLogoCid(shop.logoCid || "");
      setDescriptionCid(shop.descriptionCid || "");
      setLat(shop.location?.lat?.toString() || "");
      setLng(shop.location?.lng?.toString() || "");
    }
  }, [shop]);

  if (!currentEntityId) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">{tc("selectEntity")}</div>;
  }

  if (isLoading) {
    return <div className="flex h-full items-center justify-center"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>;
  }

  if (!shop) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">Shop not found</div>;
  }

  const handleSave = () => {
    actions.updateShop(
      shopId,
      name !== shop.name ? name : undefined,
      logoCid !== (shop.logoCid || "") ? logoCid || null : undefined,
      descriptionCid !== (shop.descriptionCid || "") ? descriptionCid || null : undefined
    );
  };

  const handleSetLocation = () => {
    if (lat && lng) {
      actions.setLocation(shopId, Number(lat), Number(lng));
    }
  };

  const handleAddManager = () => {
    if (newManager.trim()) {
      actions.addManager(shopId, newManager.trim());
      setNewManager("");
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href={`/shops/${shopId}`}><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Edit {shop.name}</h1>
          <p className="text-muted-foreground">Update shop details and settings</p>
        </div>
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Basic Information</CardTitle>
            <CardDescription>Update shop name and IPFS content</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">Shop Name</label>
              <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="Shop name" />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Logo CID</label>
              <Input value={logoCid} onChange={(e) => setLogoCid(e.target.value)} placeholder="IPFS CID for logo" />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Description CID</label>
              <Input value={descriptionCid} onChange={(e) => setDescriptionCid(e.target.value)} placeholder="IPFS CID for description" />
            </div>
            <TxButton onClick={handleSave} txStatus={actions.txState.status}>
              <Save className="mr-2 h-4 w-4" />{tc("save")}
            </TxButton>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><MapPin className="h-5 w-5" />Location</CardTitle>
            <CardDescription>Update physical location coordinates</CardDescription>
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
            <TxButton onClick={handleSetLocation} txStatus={actions.txState.status} disabled={!lat || !lng}>
              <MapPin className="mr-2 h-4 w-4" />Update Location
            </TxButton>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Managers</CardTitle>
          <CardDescription>Add or remove shop managers</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-end gap-4">
            <div className="flex-1 space-y-2">
              <label className="text-sm font-medium">New Manager Address</label>
              <Input value={newManager} onChange={(e) => setNewManager(e.target.value)} placeholder="5xxx..." />
            </div>
            <TxButton onClick={handleAddManager} txStatus={actions.txState.status} disabled={!newManager.trim()}>
              <UserPlus className="mr-2 h-4 w-4" />Add
            </TxButton>
          </div>

          {shop.managers.length === 0 ? (
            <p className="text-sm text-muted-foreground">No managers assigned</p>
          ) : (
            <div className="space-y-2">
              {shop.managers.map((addr) => (
                <div key={addr} className="flex items-center justify-between rounded-lg border p-3">
                  <AddressDisplay address={addr} />
                  <Button variant="ghost" size="icon" onClick={() => actions.removeManager(shopId, addr)}>
                    <Trash2 className="h-4 w-4 text-destructive" />
                  </Button>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-destructive">Danger Zone</CardTitle>
        </CardHeader>
        <CardContent>
          <Button variant="destructive" onClick={() => actions.closeShop(shopId)}>Close Shop</Button>
        </CardContent>
      </Card>

      {actions.txState.status === "finalized" && (
        <p className="text-sm text-green-600">Changes saved successfully!</p>
      )}
      {actions.txState.status === "error" && (
        <p className="text-sm text-destructive">{actions.txState.error}</p>
      )}
    </div>
  );
}
