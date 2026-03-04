"use client";

import { use, useState, useEffect } from "react";
import { useProduct, useProductActions } from "@/hooks/useProducts";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { Badge } from "@/components/ui/badge";
import { TxButton } from "@/components/shared/TxButton";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { formatBalance } from "@/lib/utils";
import { ArrowLeft, Save, Eye, EyeOff, Trash2 } from "lucide-react";
import Link from "next/link";

export default function ProductEditPage({ params }: { params: Promise<{ shopId: string; productId: string }> }) {
  const { shopId: shopIdStr, productId: productIdStr } = use(params);
  const shopId = Number(shopIdStr);
  const productId = Number(productIdStr);
  const { product, isLoading } = useProduct(productId);
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
            {product.isDigital && <Badge variant="secondary">Digital</Badge>}
          </div>
          <p className="text-muted-foreground">Shop #{shopId} &middot; {product.salesCount} sales</p>
        </div>
        <div className="flex gap-2">
          {product.status === "Active" ? (
            <Button variant="outline" onClick={() => actions.deactivateProduct(productId)}>
              <EyeOff className="mr-2 h-4 w-4" />Deactivate
            </Button>
          ) : (
            <Button variant="outline" onClick={() => actions.activateProduct(productId)}>
              <Eye className="mr-2 h-4 w-4" />Activate
            </Button>
          )}
          <Button variant="destructive" onClick={() => actions.deleteProduct(productId)}>
            <Trash2 className="mr-2 h-4 w-4" />Delete
          </Button>
        </div>
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Product Content</CardTitle>
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
            <div className="rounded-lg border p-3 space-y-2">
              <div className="flex justify-between text-sm"><span className="text-muted-foreground">Created</span><span>Block #{product.createdAt}</span></div>
              <div className="flex justify-between text-sm"><span className="text-muted-foreground">Total Sales</span><span>{product.salesCount}</span></div>
              <div className="flex justify-between text-sm"><span className="text-muted-foreground">Digital</span><span>{product.isDigital ? "Yes" : "No"}</span></div>
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
