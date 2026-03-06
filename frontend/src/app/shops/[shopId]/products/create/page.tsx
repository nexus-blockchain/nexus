"use client";

import { use, useState } from "react";
import { useRouter } from "next/navigation";
import { useProductActions } from "@/hooks/useProducts";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { TxButton } from "@/components/shared/TxButton";
import { PRODUCT_CATEGORIES } from "@/lib/constants";
import { ArrowLeft, Package, Eye, Tag } from "lucide-react";
import Link from "next/link";

const VISIBILITY_OPTIONS = [
  { value: "Public", label: "Public", desc: "Visible to all users" },
  { value: "MembersOnly", label: "Members Only", desc: "Only entity members can view" },
  { value: "Private", label: "Private", desc: "Hidden, accessible by direct link only" },
  { value: "Unlisted", label: "Unlisted", desc: "Not in search results, but accessible" },
] as const;

export default function CreateProductPage({ params }: { params: Promise<{ shopId: string }> }) {
  const { shopId: shopIdStr } = use(params);
  const shopId = Number(shopIdStr);
  const router = useRouter();
  const actions = useProductActions();

  const [nameCid, setNameCid] = useState("");
  const [imagesCid, setImagesCid] = useState("");
  const [detailCid, setDetailCid] = useState("");
  const [price, setPrice] = useState("");
  const [usdtPrice, setUsdtPrice] = useState("");
  const [stock, setStock] = useState("");
  const [category, setCategory] = useState("Physical");
  const [visibility, setVisibility] = useState("Public");
  const [minOrderQty, setMinOrderQty] = useState("1");
  const [maxOrderQty, setMaxOrderQty] = useState("0");

  const handleCreate = async () => {
    if (!nameCid.trim() || !price) return;
    await actions.createProduct(
      shopId,
      nameCid,
      imagesCid,
      detailCid,
      BigInt(price),
      Number(usdtPrice || 0),
      Number(stock || 0),
      category,
      visibility,
      Number(minOrderQty || 1),
      Number(maxOrderQty || 0),
    );
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href={`/shops/${shopId}/products`}><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Add Product</h1>
          <p className="text-muted-foreground">Create a new product for Shop #{shopId}</p>
        </div>
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><Package className="h-5 w-5" />Product Information</CardTitle>
            <CardDescription>IPFS content identifiers for product data</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">Name CID *</label>
              <Input value={nameCid} onChange={(e) => setNameCid(e.target.value)} placeholder="IPFS CID for product name/title" />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Images CID</label>
              <Input value={imagesCid} onChange={(e) => setImagesCid(e.target.value)} placeholder="IPFS CID for product images" />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Detail CID</label>
              <Input value={detailCid} onChange={(e) => setDetailCid(e.target.value)} placeholder="IPFS CID for product details" />
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Pricing & Inventory</CardTitle>
            <CardDescription>Set price, stock, and order quantity limits</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">Price (NEX) *</label>
              <Input type="number" value={price} onChange={(e) => setPrice(e.target.value)} placeholder="0" min="0" />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">USDT Price (optional)</label>
              <Input type="number" value={usdtPrice} onChange={(e) => setUsdtPrice(e.target.value)} placeholder="0.00" min="0" step="0.01" />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Stock Quantity</label>
              <Input type="number" value={stock} onChange={(e) => setStock(e.target.value)} placeholder="0 = unlimited" min="0" />
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">Min Order Qty</label>
                <Input type="number" value={minOrderQty} onChange={(e) => setMinOrderQty(e.target.value)} min="1" />
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">Max Order Qty</label>
                <Input type="number" value={maxOrderQty} onChange={(e) => setMaxOrderQty(e.target.value)} placeholder="0 = no limit" min="0" />
              </div>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><Tag className="h-5 w-5" />Category</CardTitle>
            <CardDescription>Choose the product category that best fits</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="grid grid-cols-2 gap-2">
              {PRODUCT_CATEGORIES.map((cat) => (
                <button
                  key={cat}
                  onClick={() => setCategory(cat)}
                  className={`rounded-lg border p-3 text-left text-sm transition-colors ${
                    category === cat
                      ? "border-primary bg-primary/5 font-medium text-primary"
                      : "border-border hover:border-primary/50 hover:bg-accent"
                  }`}
                >
                  {cat}
                </button>
              ))}
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><Eye className="h-5 w-5" />Visibility</CardTitle>
            <CardDescription>Control who can see this product</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="space-y-2">
              {VISIBILITY_OPTIONS.map((opt) => (
                <button
                  key={opt.value}
                  onClick={() => setVisibility(opt.value)}
                  className={`flex w-full flex-col rounded-lg border p-3 text-left transition-colors ${
                    visibility === opt.value
                      ? "border-primary bg-primary/5"
                      : "border-border hover:border-primary/50 hover:bg-accent"
                  }`}
                >
                  <span className={`text-sm ${visibility === opt.value ? "font-medium text-primary" : ""}`}>{opt.label}</span>
                  <span className="text-xs text-muted-foreground">{opt.desc}</span>
                </button>
              ))}
            </div>
          </CardContent>
        </Card>
      </div>

      <div className="flex items-center gap-4">
        <TxButton onClick={handleCreate} txStatus={actions.txState.status} disabled={!nameCid.trim() || !price}>
          <Package className="mr-2 h-4 w-4" />Create Product
        </TxButton>
        <Button variant="outline" asChild>
          <Link href={`/shops/${shopId}/products`}>Cancel</Link>
        </Button>
      </div>

      {actions.txState.status === "finalized" && (
        <div className="rounded-lg border border-green-200 bg-green-50 p-4 dark:border-green-800 dark:bg-green-950">
          <p className="text-sm text-green-800 dark:text-green-200">Product created successfully!</p>
          <Button variant="link" className="mt-1 h-auto p-0 text-green-700 dark:text-green-300" onClick={() => router.push(`/shops/${shopId}/products`)}>
            Back to Products
          </Button>
        </div>
      )}
      {actions.txState.status === "error" && (
        <p className="text-sm text-destructive">{actions.txState.error}</p>
      )}
    </div>
  );
}
