"use client";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
import { StatusBadge } from "@/components/shared/StatusBadge";
import { ShoppingCart, XCircle, RotateCcw, ArrowLeft } from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

const PLACEHOLDER_ORDERS = [
  { id: 101, side: "Buy", amount: "500.00", price: "1.03", status: "Open" },
  { id: 102, side: "Sell", amount: "200.00", price: "1.06", status: "Open" },
  { id: 98, side: "Buy", amount: "1,000.00", price: "1.02", status: "PartiallyFilled" },
  { id: 95, side: "Sell", amount: "750.00", price: "1.05", status: "Filled" },
  { id: 90, side: "Buy", amount: "300.00", price: "1.01", status: "Cancelled" },
];

export default function TradingMyOrdersPage() {
  const t = useTranslations("common");

  const statusVariant = (status: string) => {
    switch (status) {
      case "Open": return "default" as const;
      case "Filled": return "secondary" as const;
      case "PartiallyFilled": return "outline" as const;
      case "Cancelled": return "destructive" as const;
      default: return "outline" as const;
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/trading"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight">My Orders</h1>
          <p className="text-muted-foreground">Manage your active and historical NEX/USDT orders</p>
        </div>
        <Button variant="outline" size="sm">
          <RotateCcw className="mr-2 h-3 w-3" />Refresh
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-4">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Open</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">2</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Partially Filled</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">1</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Filled</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">1</p></CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Cancelled</CardTitle></CardHeader>
          <CardContent><p className="text-2xl font-bold">1</p></CardContent>
        </Card>
      </div>

      {PLACEHOLDER_ORDERS.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <ShoppingCart className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No orders yet</p>
            <p className="text-sm text-muted-foreground">Place your first order on the trading page</p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>ID</TableHead>
                <TableHead>Side</TableHead>
                <TableHead className="text-right">Amount (NEX)</TableHead>
                <TableHead className="text-right">Price (USDT)</TableHead>
                <TableHead>Status</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {PLACEHOLDER_ORDERS.map((order) => (
                <TableRow key={order.id}>
                  <TableCell className="font-mono">#{order.id}</TableCell>
                  <TableCell>
                    <Badge variant={order.side === "Buy" ? "default" : "secondary"}>
                      {order.side}
                    </Badge>
                  </TableCell>
                  <TableCell className="text-right font-mono">{order.amount}</TableCell>
                  <TableCell className="text-right font-mono">{order.price}</TableCell>
                  <TableCell>
                    <Badge variant={statusVariant(order.status)}>{order.status}</Badge>
                  </TableCell>
                  <TableCell className="text-right">
                    {order.status === "Open" && (
                      <Button variant="ghost" size="sm">
                        <XCircle className="mr-1 h-3 w-3" />Cancel
                      </Button>
                    )}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Card>
      )}
    </div>
  );
}
