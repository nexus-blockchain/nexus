"use client";

import { use, useState, useEffect } from "react";
import { useProduct, useProductActions } from "@/hooks/useProducts";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { TxButton } from "@/components/shared/TxButton";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { PRODUCT_CATEGORIES, PRODUCT_STATUS } from "@/lib/constants";
import { ArrowLeft, Save, Eye, EyeOff, Trash2, Tag, ShieldCheck, Package } from "lucide-react";
import Link from "next/link";

const VISIBILITY_OPTIONS = [
  { value: "Public", label: "Public" },
  { value: "MembersOnly", label: "Members Only" },
  { value: "Private", label: "Private" },
  { value: "Unlisted", label: "Unlisted" },
] as const;

const STATUS_FLOW: Record<string, string[]> = {
  Draft: ["OnSale"],
  OnSale: ["OffShelf"],
  OffShelf: ["OnSale"],
  SoldOut: ["OnSale"],
};

export default function ProductEditPage({ params }: { params: Promise<{ shopId: string; productId: string }> }) {
  const { shopId: shopIdStr, productId: productIdStr } = use(params);
  const shopId = Number(shopIdStr);
  const productId = Number(productIdStr);
  const { product, isLoading, refetch } = useProduct(productId);
  const actions = useProductActions();

  const [nameCid, setNameCid] = useState("");
  const [imagesCid, setImagesCid] = useState("");
  const [detailCid, setDetailCid] = useState("");
  const [price, setPrice] = useState("");
  const [stock, setStock] = useState("");

  useEffect(() => {
    if (product) {
      setNameCid(product.nameCid || "");
      setImagesCid(product.imagesCid || "");
      setDetailCid(product.detailCid || "");
      setPrice(product.price.toString());
      setStock(product.stock.toString());
    }
  }, [product]);

  if (isLoading) {
    return <div className="flex h-full items-center justify-center"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>;
  }

  if (!product) {
    return <div className="flex h-full items-center justify-center text-muted-foreground">Product not found</div>;
  }

  const handleSave = () => {
    actions.updateProduct(
      productId,
      nameCid !== product.nameCid ? nameCid : undefined,
      imagesCid !== product.imagesCid ? imagesCid : undefined,
      detailCid !== product.detailCid ? detailCid : undefined,
      price !== product.price.toString() ? BigInt(price) : undefined,
      stock !== product.stock.toString() ? Number(stock) : undefined
    );
  };

  const nextStatuses = STATUS_FLOW[product.status] || [];

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href={`/shops/${shopId}/products`}><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <div className="flex items-center gap-3">
            <h1 className="text-3xl font-bold tracking-tight">Product #{productId}</h1>
            <StatusBadge status={product.status} />
            <Badge variant="secondary">{product.category}</Badge>
            <Badge variant="outline">{product.visibility}</Badge>
          </div>
          <p className="text-muted-foreground">Shop #{shopId} &middot; {product.salesCount} sales</p>
        </div>
        <div className="flex gap-2">
          {nextStatuses.includes("OnSale") && (
            <Button variant="outline" onClick={() => { actions.activateProduct(productId); }}>
              <Eye className="mr-2 h-4 w-4" />Put On Sale
            </Button>
          )}
          {nextStatuses.includes("OffShelf") && (
            <Button variant="outline" onClick={() => { actions.deactivateProduct(productId); }}>
              <EyeOff className="mr-2 h-4 w-4" />Take Off Shelf
            </Button>
          )}
          <Button variant="destructive" size="icon" onClick={() => actions.deleteProduct(productId)} title="Delete">
            <Trash2 className="h-4 w-4" />
          </Button>
        </div>
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><Package className="h-5 w-5" />Product Content</CardTitle>
            <CardDescription>IPFS content identifiers</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">Name CID</label>
              <Input value={nameCid} onChange={(e) => setNameCid(e.target.value)} placeholder="IPFS CID" />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Images CID</label>
              <Input value={imagesCid} onChange={(e) => setImagesCid(e.target.value)} placeholder="IPFS CID" />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Detail CID</label>
              <Input value={detailCid} onChange={(e) => setDetailCid(e.target.value)} placeholder="IPFS CID" />
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Pricing & Stock</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">Price (NEX)</label>
              <Input type="number" value={price} onChange={(e) => setPrice(e.target.value)} min="0" />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">USDT Price</label>
              <Input value={product.usdtPrice} disabled />
              <p className="text-xs text-muted-foreground">USDT price is set at creation</p>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Stock</label>
              <Input type="number" value={stock} onChange={(e) => setStock(e.target.value)} min="0" />
            </div>
            <Separator />
            <div className="space-y-2 rounded-lg border p-3">
              <div className="flex justify-between text-sm"><span className="text-muted-foreground">Created</span><span>Block #{product.createdAt}</span></div>
              <div className="flex justify-between text-sm"><span className="text-muted-foreground">Total Sales</span><span>{product.salesCount}</span></div>
              <div className="flex justify-between text-sm"><span className="text-muted-foreground">Min Order</span><span>{product.minOrderQty}</span></div>
              <div className="flex justify-between text-sm"><span className="text-muted-foreground">Max Order</span><span>{product.maxOrderQty || "No limit"}</span></div>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><Tag className="h-5 w-5" />Category</CardTitle>
            <CardDescription>Change the product category</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="grid grid-cols-3 gap-2">
              {PRODUCT_CATEGORIES.map((cat) => (
                <button
                  key={cat}
                  onClick={() => actions.setProductCategory(productId, cat)}
                  className={`rounded-lg border p-2.5 text-sm transition-colors ${
                    product.category === cat
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
            <CardTitle className="flex items-center gap-2"><ShieldCheck className="h-5 w-5" />Visibility</CardTitle>
            <CardDescription>Control who can see this product</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="space-y-2">
              {VISIBILITY_OPTIONS.map((opt) => (
                <button
                  key={opt.value}
                  onClick={() => actions.setProductVisibility(productId, opt.value)}
                  className={`flex w-full rounded-lg border p-3 text-left text-sm transition-colors ${
                    product.visibility === opt.value
                      ? "border-primary bg-primary/5 font-medium text-primary"
                      : "border-border hover:border-primary/50 hover:bg-accent"
                  }`}
                >
                  {opt.label}
                </button>
              ))}
            </div>
          </CardContent>
        </Card>
      </div>

      <TxButton onClick={handleSave} txStatus={actions.txState.status}>
        <Save className="mr-2 h-4 w-4" />Save Changes
      </TxButton>

      {actions.txState.status === "finalized" && <p className="text-sm text-green-600">Changes saved!</p>}
      {actions.txState.status === "error" && <p className="text-sm text-destructive">{actions.txState.error}</p>}
    </div>
  );
}
