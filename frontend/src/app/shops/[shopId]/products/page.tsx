"use client";

import { use } from "react";
import { useProducts, useProductActions } from "@/hooks/useProducts";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { formatBalance } from "@/lib/utils";
import { Package, Plus, ArrowLeft, Eye, EyeOff, Trash2 } from "lucide-react";
import Link from "next/link";

export default function ProductsPage({ params }: { params: Promise<{ shopId: string }> }) {
  const { shopId: shopIdStr } = use(params);
  const shopId = Number(shopIdStr);
  const { products, isLoading, refetch } = useProducts(shopId);
  const actions = useProductActions();

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href={`/shops/${shopId}`}><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight">Products</h1>
          <p className="text-muted-foreground">Manage products for Shop #{shopId}</p>
        </div>
        <Button asChild>
          <Link href={`/shops/${shopId}/products/create`}><Plus className="mr-2 h-4 w-4" />Add Product</Link>
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Products</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">{products.length}</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Active</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">{products.filter((p) => p.status === "Active").length}</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Sales</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">{products.reduce((sum, p) => sum + p.salesCount, 0)}</p></CardContent>
        </Card>
      </div>

      {isLoading ? (
        <div className="flex justify-center py-12"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>
      ) : products.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Package className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No Products Yet</p>
            <p className="text-sm text-muted-foreground">Add your first product to start selling.</p>
            <Button className="mt-4" asChild>
              <Link href={`/shops/${shopId}/products/create`}><Plus className="mr-2 h-4 w-4" />Add Product</Link>
            </Button>
          </CardContent>
        </Card>
      ) : (
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {products.map((product) => (
            <Card key={product.id} className="hover:shadow-md transition-shadow">
              <CardHeader className="pb-3">
                <div className="flex items-center justify-between">
                  <CardTitle className="text-base">Product #{product.id}</CardTitle>
                  <StatusBadge status={product.status} />
                </div>
              </CardHeader>
              <CardContent className="space-y-3">
                <div className="flex items-center justify-between text-sm">
                  <span className="text-muted-foreground">Price</span>
                  <span className="font-semibold">{formatBalance(product.price)} NEX</span>
                </div>
                {product.usdtPrice > 0 && (
                  <div className="flex items-center justify-between text-sm">
                    <span className="text-muted-foreground">USDT Price</span>
                    <span className="font-semibold">${product.usdtPrice}</span>
                  </div>
                )}
                <div className="flex items-center justify-between text-sm">
                  <span className="text-muted-foreground">Stock</span>
                  <span className={product.stock === 0 ? "text-destructive font-semibold" : ""}>{product.stock === 0 ? "Out of stock" : product.stock}</span>
                </div>
                <div className="flex items-center justify-between text-sm">
                  <span className="text-muted-foreground">Sales</span>
                  <span>{product.salesCount}</span>
                </div>
                <div className="flex items-center gap-1">
                  {product.isDigital && <Badge variant="secondary" className="text-xs">Digital</Badge>}
                </div>
                <div className="flex gap-2 pt-2">
                  <Button variant="outline" size="sm" asChild className="flex-1">
                    <Link href={`/shops/${shopId}/products/${product.id}`}>Edit</Link>
                  </Button>
                  {product.status === "Active" ? (
                    <Button variant="ghost" size="icon" onClick={() => { actions.deactivateProduct(product.id); }} title="Deactivate">
                      <EyeOff className="h-4 w-4" />
                    </Button>
                  ) : (
                    <Button variant="ghost" size="icon" onClick={() => { actions.activateProduct(product.id); }} title="Activate">
                      <Eye className="h-4 w-4" />
                    </Button>
                  )}
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      )}

      {actions.txState.status === "finalized" && (
        <p className="text-sm text-green-600">Action completed successfully!</p>
      )}
      {actions.txState.status === "error" && (
        <p className="text-sm text-destructive">{actions.txState.error}</p>
      )}
    </div>
  );
}
